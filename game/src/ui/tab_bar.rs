use bevy::{
    ecs::{
        bundle::{BundleEffect, DynamicBundle},
        component::{
            ComponentId, Components, ComponentsRegistrator, HookContext, RequiredComponents,
            StorageType,
        },
        world::DeferredWorld,
    },
    prelude::*,
    ptr::OwningPtr,
};

use super::{
    ObservedBy,
    button::{ButtonStyling, DEFAULT_HOVER_COLOR, HoveredButton, NormalButton, PressedButton},
    style::{
        ComponentEquals, ConditionalStyle, DefaultStyle, HasComponent, StyleRef, Styles,
        style_label,
    },
};

pub fn client(app: &mut App) {
    app.add_systems(Startup, setup_tabbar_style);
}

style_label!(UnfocusedTabBarButton);
style_label!(FocusedTabBarButton);

fn setup_tabbar_style(mut styles: ResMut<Styles>) {
    let style = styles
        .new_style_from(NormalButton, UnfocusedTabBarButton)
        .unwrap();
    style.background_color = Some(UNFOCUSED_COLOR);

    let style = styles
        .new_style_from(NormalButton, FocusedTabBarButton)
        .unwrap();
    style.background_color = Some(FOCUSED_COLOR);
}

pub struct TabBarBuilder {
    tabs: Vec<(
        String,
        Box<dyn (for<'a, 'b> FnOnce(&'b mut ChildSpawner<'a>) -> EntityWorldMut<'b>) + Send + Sync>,
    )>,
}

impl TabBarBuilder {
    pub fn new() -> Self {
        Self { tabs: vec![] }
    }

    pub fn tab(mut self, title: impl Into<String>, contents: impl Bundle) -> Self {
        self.tabs
            .push((title.into(), Box::new(|spawner| spawner.spawn(contents))));
        self
    }

    pub fn tab_builder(
        mut self,
        title: impl Into<String>,
        contents: impl (for<'a, 'b> FnOnce(&'b mut ChildSpawner<'a>) -> EntityWorldMut<'b>)
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.tabs.push((title.into(), Box::new(contents)));
        self
    }
}

impl Default for TabBarBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub fn tab_bar() -> TabBarBuilder {
    TabBarBuilder::new()
}

unsafe impl Bundle for TabBarBuilder {
    fn component_ids(_components: &mut ComponentsRegistrator, _ids: &mut impl FnMut(ComponentId)) {}

    fn get_component_ids(_components: &Components, _ids: &mut impl FnMut(Option<ComponentId>)) {}

    fn register_required_components(
        _components: &mut ComponentsRegistrator,
        _required_components: &mut RequiredComponents,
    ) {
    }
}

impl DynamicBundle for TabBarBuilder {
    type Effect = Self;

    fn get_components(self, _func: &mut impl FnMut(StorageType, OwningPtr<'_>)) -> Self::Effect {
        self
    }
}

#[derive(Component)]
#[component(
    on_insert = hide(),
    on_remove = show(),
)]
pub struct Hidden;

fn hide() -> impl Fn(DeferredWorld, HookContext) {
    |mut world, ctx| {
        if let Some(mut node) = world.entity_mut(ctx.entity).get_mut::<Node>() {
            node.display = Display::None;
        }
    }
}
fn show() -> impl Fn(DeferredWorld, HookContext) {
    |mut world, ctx| {
        if let Some(mut node) = world.entity_mut(ctx.entity).get_mut::<Node>() {
            node.display = Display::Flex;
        }
    }
}

#[derive(Component)]
#[relationship(relationship_target = ControlsTab)]
pub struct TabControlledBy(Entity);
#[derive(Component)]
#[relationship_target(relationship = TabControlledBy)]
pub struct ControlsTab(Entity);

#[derive(Component)]
#[relationship(relationship_target = TabGroup)]
pub struct TabBelongsToGroup(Entity);
#[derive(Component)]
#[relationship_target(relationship = TabBelongsToGroup)]
pub struct TabGroup(Vec<Entity>);

const FOCUSED_COLOR: Color = Color::srgb(0.3, 0.3, 0.3);
const UNFOCUSED_COLOR: Color = Color::srgb(0.2, 0.2, 0.2);

const FOCUSED_STYLING: ButtonStyling = ButtonStyling {
    normal_color: FOCUSED_COLOR,
    border: false,
    ..ButtonStyling::DEFAULT
};
const UNFOCUSED_STYLING: ButtonStyling = ButtonStyling {
    normal_color: UNFOCUSED_COLOR,
    border: false,
    ..ButtonStyling::DEFAULT
};

const FOCUSED: (BackgroundColor, ButtonStyling) = (BackgroundColor(FOCUSED_COLOR), FOCUSED_STYLING);
const UNFOCUSED: (BackgroundColor, ButtonStyling) =
    (BackgroundColor(UNFOCUSED_COLOR), UNFOCUSED_STYLING);

#[derive(Component)]
struct FocusedTab;

impl BundleEffect for TabBarBuilder {
    fn apply(self, entity: &mut EntityWorldMut) {
        let group = entity.id();
        let mut first_tab = Entity::PLACEHOLDER;
        entity.insert((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(UNFOCUSED_COLOR),
        ));

        entity.with_children(|spawner| {
            let (titles, contents): (Vec<_>, Vec<_>) = self.tabs.into_iter().unzip();

            let mut tabs = vec![];

            spawner
                .spawn(Node { ..default() })
                .with_children(|spawner| {
                    let mut first = true;
                    for title in titles {
                        let mut child = spawner.spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(10.0), Val::Px(0.0)),
                                ..default()
                            },
                            Text::new(&title),
                            TextLayout::new_with_justify(JustifyText::Center),
                            Button,
                            ObservedBy::new(activate_tab),
                            TabBelongsToGroup(group),
                            BackgroundColor(Color::WHITE),
                            ConditionalStyle::new()
                                .when(
                                    (
                                        ComponentEquals(Interaction::None),
                                        HasComponent::<FocusedTab>::new(),
                                    ),
                                    FocusedTabBarButton,
                                )
                                .when(ComponentEquals(Interaction::None), UnfocusedTabBarButton)
                                .when(ComponentEquals(Interaction::Hovered), HoveredButton)
                                .when(ComponentEquals(Interaction::Pressed), PressedButton),
                        ));
                        if first {
                            first = false;
                            child.insert(FocusedTab);
                        }
                        let id = child.id();
                        tabs.push(id);
                    }
                });

            let mut first = true;

            spawner
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        flex_grow: 1.0,
                        flex_basis: Val::Px(0.0),
                        ..default()
                    },
                    BackgroundColor(FOCUSED_COLOR),
                ))
                .with_children(|spawner| {
                    for (content, tab) in contents.into_iter().zip(tabs) {
                        let mut child = content(spawner);
                        child.insert(TabControlledBy(tab));
                        if !first {
                            child.insert(Hidden);
                        } else {
                            first_tab = child.id();
                            first = false;
                        }
                    }
                });
        });

        entity.insert((ActiveTab(first_tab),));
    }
}

#[derive(Component)]
pub struct ActiveTab(Entity);

fn activate_tab(
    mut trigger: Trigger<Pointer<Click>>,
    clicked: Query<(&ControlsTab, &TabBelongsToGroup)>,
    mut tab_group: Query<&mut ActiveTab>,
    tab_contents: Query<&TabControlledBy>,
    mut tab_style_ref: Query<&mut StyleRef>,
    mut commands: Commands,
) {
    // commands.entity(trigger.target()).insert(FOCUSED);

    let (tab, group) = clicked.get(trigger.target()).unwrap();
    let mut active_tab = tab_group.get_mut(group.0).unwrap();

    // commands
    //     .entity(tab_contents.get(active_tab.0).unwrap().0)
    //     .insert(UNFOCUSED);
    // tab_style_ref
    //     .get_mut(tab_contents.get(active_tab.0).unwrap().0)
    //     .unwrap()
    //     .0 = "UnfocusedTabBarButton".into();
    // tab_style_ref.get_mut(trigger.target()).unwrap().0 = "FocusedTabBarButton".into();

    commands
        .entity(tab_contents.get(active_tab.0).unwrap().0)
        .remove::<FocusedTab>();
    commands.entity(trigger.target()).insert(FocusedTab);

    commands.entity(active_tab.0).insert(Hidden);
    commands.entity(tab.0).remove::<Hidden>();
    active_tab.0 = tab.0;

    trigger.propagate(false);
}
