use std::marker::PhantomData;

use bevy::{platform::collections::HashMap, prelude::*, reflect::DynamicStruct};

pub fn client(app: &mut App) {
    app.insert_resource(Styles::new_with_default()).add_systems(
        PostUpdate,
        (update_conditional_style, update_with_style2).chain(),
    );
}

// #[derive(Resource)]
// pub struct Styles {
//     map: HashMap<String, Style>,
// }

pub trait StyleLabel {
    fn label(&self) -> &str;
}

impl StyleLabel for String {
    fn label(&self) -> &str {
        self.as_str()
    }
}

impl StyleLabel for &'_ str {
    fn label(&self) -> &str {
        self
    }
}

impl<S: StyleLabel> StyleLabel for &S {
    fn label(&self) -> &str {
        S::label(self)
    }
}

pub macro style_label($name:ident) {
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    pub struct $name;

    impl StyleLabel for $name {
        fn label(&self) -> &str {
            stringify!($name)
        }
    }

    impl From<$name> for String {
        fn from(value: $name) -> Self {
            value.label().into()
        }
    }
}

style_label!(DefaultStyle);

impl Styles {
    pub fn new_with_default() -> Self {
        Self {
            styles: HashMap::from_iter([(DefaultStyle.label().into(), Style::default())]),
        }
    }

    pub fn get(&self, name: impl StyleLabel) -> Option<&Style> {
        self.styles.get(name.label())
    }

    #[allow(dead_code)]
    pub fn get_mut(&mut self, name: impl StyleLabel) -> Option<&mut Style> {
        self.styles.get_mut(name.label())
    }

    pub fn new_style_from(
        &mut self,
        reference: impl StyleLabel,
        new_name: impl StyleLabel,
    ) -> Option<&mut Style> {
        // let reference_style = self.styles.get(reference.label())?;
        let mut new = Style::default();
        new.parent = Some(reference.label().into());
        Some(
            self.styles
                .entry(new_name.label().into())
                .insert(new)
                .into_mut(),
        )
    }

    #[allow(dead_code)]
    pub fn insert(&mut self, name: impl StyleLabel, style: Style) {
        self.styles.insert(name.label().into(), style);
    }
}

// #[derive(Debug, Clone, Default)]
// pub struct Style {
//     pub colors: Colors,
//     pub button_colors: Buttons,
//     pub border: Val,
// }

// #[derive(Debug, Clone)]
// pub struct Colors {
//     pub background: Color,
//     pub foreground: Color,
//     pub border: Color,
// }

// impl Default for Colors {
//     fn default() -> Self {
//         Self {
//             background: Color::srgb(0.3, 0.3, 0.3),
//             foreground: Color::srgb(1.0, 1.0, 1.0),
//             border: Color::WHITE,
//         }
//     }
// }

// #[derive(Debug, Clone)]
// pub struct Buttons {
//     pub background: Color,
//     pub hover: Color,
//     pub press: Color,
// }

// impl Default for Buttons {
//     fn default() -> Self {
//         Self {
//             background: Color::srgb(0.3, 0.3, 0.3),
//             hover: Color::srgb(0.4, 0.4, 0.4),
//             press: Color::srgb(0.2, 0.2, 0.2),
//         }
//     }
// }

#[derive(Clone, Component, Deref, PartialEq, Eq, Debug)]
pub struct StyleRef(pub String);

impl StyleRef {
    pub fn new(label: impl StyleLabel) -> Self {
        Self(label.label().into())
    }
}

impl Default for StyleRef {
    fn default() -> Self {
        Self::new(DefaultStyle)
    }
}

// pub fn update_with_style(styles: Res<Styles>,
//     mut all: Query<(Entity, &StyleRef), Changed<StyleRef>>,
//     mut buttons: Query<(Entity, &Interaction, &StyleRef), (Or<(Changed<Interaction>, Changed<StyleRef>)>, With<Button>)>,
//     mut background: Query<&mut BackgroundColor>,
//     mut border_color: Query<&mut BorderColor>,
//     mut node: Query<&mut Node>,
// ) {
//     let mut bg_changes = vec![];
//     let mut border_color_changes = vec![];
//     // let mut border_size_changes: vec![];

//     for (e, style_ref) in &mut all {
//         let style = styles.get(&style_ref.0).unwrap();
//         bg_changes.push((e, style.colors.background));
//         border_color_changes.push((e, style.colors.border));
//     }

//     for (e, interaction, style_ref) in &mut buttons {
//         let style = styles.get(&style_ref.0).unwrap();
//         let color = match interaction {
//             Interaction::Pressed => style.button_colors.press,
//             Interaction::Hovered => style.button_colors.hover,
//             Interaction::None => style.button_colors.background,
//         };
//         bg_changes.push((e, color));
//     }

//     for (e, color) in bg_changes {
//         if let Ok(mut bg) = background.get_mut(e) {
//             bg.0 = color;
//         }
//     }
// }

#[derive(Resource)]
pub struct Styles {
    styles: HashMap<String, Style>,
}

#[derive(Debug, Default, Component)]
pub struct Style {
    pub parent: Option<String>,
    pub node_stuff: DynamicStruct,
    pub background_color: Option<Color>,
    pub border_color: Option<Color>,
}

impl PartialEq for Style {
    fn eq(&self, other: &Self) -> bool {
        self.parent == other.parent
            && self
                .node_stuff
                .reflect_partial_eq(&other.node_stuff)
                .unwrap()
            && self.background_color == other.background_color
            && self.border_color == other.border_color
    }
}

impl Style {
    pub fn set_node_field<T: PartialReflect>(&mut self, name: &str, value: T) {
        self.node_stuff.insert(name, value);
    }
    #[allow(dead_code)]

    pub fn apply(&mut self, child: &Style) {
        for (i, field) in child.node_stuff.iter_fields().enumerate() {
            let name = child.node_stuff.name_at(i).unwrap();
            self.node_stuff.insert_boxed(
                name,
                field
                    .reflect_clone()
                    .map(|x| x as Box<dyn PartialReflect>)
                    .unwrap_or_else(|_| field.to_dynamic()),
            );
        }
        self.background_color = child.background_color.or(self.background_color);
        self.border_color = child.border_color.or(self.border_color);
    }

    pub fn is_empty(&self) -> bool {
        self.background_color.is_none()
            && self.border_color.is_none()
            && self.node_stuff.field_len() == 0
    }
}

impl Clone for Style {
    fn clone(&self) -> Self {
        Self {
            parent: self.parent.clone(),
            node_stuff: self.node_stuff.to_dynamic_struct(),
            background_color: self.background_color.clone(),
            border_color: self.border_color.clone(),
        }
    }
}

#[derive(Component)]
pub struct ConditionalStyle {
    pub x: Vec<Box<dyn StyleExtractor>>,
}

impl Clone for ConditionalStyle {
    fn clone(&self) -> Self {
        Self {
            x: self.x.iter().map(|x| x.boxed_clone()).collect(),
        }
    }
}

impl ConditionalStyle {
    pub fn new() -> Self {
        Self { x: vec![] }
    }

    pub fn with(mut self, cond: impl StyleExtractor) -> Self {
        self.x.push(Box::new(cond));
        self
    }

    pub fn when(
        self,
        cond: impl StyleCondition,
        label: impl StyleLabel + Send + Sync + 'static,
    ) -> Self {
        let style_ref = StyleRef::new(label);
        self.with(move |e: EntityRef<'_>| cond.test(e).then(|| style_ref.clone()))
    }
}

pub trait StyleExtractor: Send + Sync + 'static {
    fn extract(&self, entity: EntityRef) -> Option<StyleRef>;
    fn boxed_clone(&self) -> Box<dyn StyleExtractor>;
}

impl<F: Fn(EntityRef) -> Option<StyleRef> + Send + Sync + 'static + Clone> StyleExtractor for F {
    fn extract(&self, entity: EntityRef) -> Option<StyleRef> {
        self(entity)
    }

    fn boxed_clone(&self) -> Box<dyn StyleExtractor> {
        Box::new(self.clone())
    }
}

impl<C: StyleCondition> StyleExtractor for (C, StyleRef) {
    fn extract(&self, entity: EntityRef) -> Option<StyleRef> {
        self.0.test(entity).then(|| self.1.clone())
    }

    fn boxed_clone(&self) -> Box<dyn StyleExtractor> {
        Box::new(self.clone())
    }
}

pub trait StyleCondition: Clone + Send + Sync + 'static {
    fn test(&self, entity: EntityRef) -> bool;
}

impl<F: Fn(EntityRef) -> bool + Send + Sync + 'static + Clone> StyleCondition for F {
    fn test(&self, entity: EntityRef) -> bool {
        self(entity)
    }
}

#[derive(Clone)]
pub struct ComponentEquals<C: Component + Clone + PartialEq>(pub C);

impl<C: Component + Clone + PartialEq> StyleCondition for ComponentEquals<C> {
    fn test(&self, entity: EntityRef) -> bool {
        entity.get::<C>() == Some(&self.0)
    }
}

pub struct HasComponent<C: Component> {
    _phantom: PhantomData<C>,
}

impl<C: Component> HasComponent<C> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }
}

impl<C: Component> Clone for HasComponent<C> {
    fn clone(&self) -> Self {
        Self {
            _phantom: self._phantom.clone(),
        }
    }
}

impl<C: Component> StyleCondition for HasComponent<C> {
    fn test(&self, entity: EntityRef) -> bool {
        entity.contains::<C>()
    }
}

impl<A: StyleCondition> StyleCondition for (A,) {
    fn test(&self, entity: EntityRef) -> bool {
        self.0.test(entity)
    }
}

impl<A: StyleCondition, B: StyleCondition> StyleCondition for (A, B) {
    fn test(&self, entity: EntityRef) -> bool {
        self.0.test(entity) && self.1.test(entity)
    }
}

pub fn update_conditional_style(world: &mut World) {
    let mut changes = vec![];

    let mut q = world.query::<(Entity, &ConditionalStyle, Option<&StyleRef>)>();
    for (e, conds, old_style_ref) in q.iter(world) {
        for cond in &conds.x {
            if let Some(style_ref) = cond.extract(world.entity(e)) {
                if Some(&style_ref) != old_style_ref {
                    changes.push((e, style_ref));
                }
                break;
            }
        }
    }

    for (e, style_ref) in changes {
        world.entity_mut(e).insert(style_ref);
    }
}

pub fn update_with_style2(
    styles: Res<Styles>,
    mut query: Query<
        (
            Entity,
            &mut Node,
            Option<&mut BackgroundColor>,
            Option<&mut BorderColor>,
            &StyleRef,
            Option<&Style>,
        ),
        Or<(Changed<StyleRef>, Changed<Style>)>,
    >,
    mut commands: Commands,
) {
    for (e, mut node, mut bg, mut bc, style_ref, style_override) in &mut query {
        let mut chain = vec![];
        if let Some(style) = style_override {
            chain.push(style);
        }

        let mut style = styles.styles.get(&style_ref.0).unwrap();
        chain.push(style);

        while let Some(parent) = &style.parent {
            style = styles.get(parent).unwrap();
            chain.push(style);
        }

        for style in chain.iter().rev() {
            node.apply(&style.node_stuff);
            if let Some(new_bg) = style.background_color {
                if let Some(bg) = bg.as_mut() {
                    bg.0 = new_bg;
                } else {
                    commands.entity(e).insert(BackgroundColor(new_bg));
                }
            }
            if let Some(new_bc) = style.border_color {
                if let Some(bc) = bc.as_mut() {
                    bc.0 = new_bc;
                } else {
                    commands.entity(e).insert(BorderColor(new_bc));
                }
            }
        }
    }
}
