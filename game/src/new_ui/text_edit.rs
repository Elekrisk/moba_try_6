use std::{fmt::Debug, marker::PhantomData};

use bevy::{
    color::palettes,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::{FocusedInput, InputFocus, IsFocused, IsFocusedHelper},
    prelude::*,
    text::ComputedTextBlock,
    window::PrimaryWindow,
};

use crate::new_ui::{View, Widget};

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (update_cursor, cursor_blink));
}

#[derive(Debug)]
pub struct TextEditView<M> {
    initial: String,
    placeholder: String,
    _marker: PhantomData<M>
}

impl<M> TextEditView<M> {
    pub fn new(initial: impl Into<String>, placeholder: impl Into<String>) -> Self {
        Self {
            initial: initial.into(),
            placeholder: placeholder.into(),
            _marker: PhantomData
        }
    }
}

impl<M: Component + Default + Debug> View for TextEditView<M> {
    type Widget = TextEditWidget;

    fn build(&mut self, parent: &mut ChildSpawnerCommands) -> Self::Widget {
        let mut text_entity = Entity::PLACEHOLDER;
        let mut cursor_entity = Entity::PLACEHOLDER;
        let entity = parent
            .spawn((
                Node {
                    padding: UiRect::all(Val::Px(5.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(Color::Srgba(palettes::tailwind::GRAY_700)),
                BorderColor(Color::WHITE),
            ))
            .with_children(|parent| {
                text_entity = parent
                    .spawn((
                        Text::new(if self.initial.is_empty() {
                            &self.placeholder
                        } else {
                            &self.initial
                        }),
                        TextEdit {
                            text: self.initial.clone(),
                            placeholder: self.placeholder.clone(),
                            byte_cursor: 0,
                        },
                        Pickable::IGNORE,
                        M::default(),
                    ))
                    .id();
                cursor_entity = parent
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Px(1.0),
                            height: Val::Percent(100.0),
                            margin: UiRect::axes(Val::Px(5.0), Val::Px(1.0)),
                            ..default()
                        },
                        TextEditCursor {
                            blink: Timer::from_seconds(0.5, TimerMode::Repeating),
                            focused: false,
                        },
                        BackgroundColor(Color::WHITE),
                        Pickable::IGNORE,
                    ))
                    .id();
            })
            .observe(input)
            .observe(mouse_input)
            .id();

        parent.commands().insert_resource(InputFocus(Some(entity)));

        TextEditWidget {
            entity,
            text_entity,
            _cursor_entity: cursor_entity,
            parent: parent.target_entity(),
        }
    }

    fn rebuild(&mut self, prev: &Self, widget: &mut Self::Widget, mut commands: Commands) {
        if self.placeholder != prev.placeholder {
            let new_placeholder = self.placeholder.clone();
            commands
                .entity(widget.text_entity)
                .entry::<TextEdit>()
                .and_modify(|mut te| te.placeholder = new_placeholder);
        }
    }

    fn name(&self) -> String {
        "TextEdit".into()
    }
}

pub struct TextEditWidget {
    entity: Entity,
    text_entity: Entity,
    _cursor_entity: Entity,
    parent: Entity,
}

impl Widget for TextEditWidget {
    fn entity(&self) -> Entity {
        self.entity
    }

    fn parent(&self) -> Entity {
        self.parent
    }
}

#[derive(Component)]
pub struct TextEdit {
    pub text: String,
    pub placeholder: String,
    byte_cursor: usize,
}

#[derive(Component)]
pub struct TextEditCursor {
    blink: Timer,
    focused: bool,
}

fn mouse_input(
    mut trigger: Trigger<Pointer<Click>>,
    q: Query<(&ComputedNode, &Children)>,
    mut q2: Query<(&ComputedTextBlock, &mut TextEdit)>,
    mut q3: Query<(&mut Node, &mut TextEditCursor)>,
    mut input_focus: ResMut<InputFocus>,
) {
    if trigger.button == PointerButton::Primary {
        trigger.propagate(false);
        input_focus.0 = Some(trigger.target());

        info!("Clicked {}", trigger.target());

        let (node, children) = q.get(trigger.target()).unwrap();
        let (text, mut edit) = q2.get_mut(children[0]).unwrap();

        let size = node.size;
        let offset = node.content_inset();
        let offset = vec2(offset.left, offset.top);

        let hit = size * trigger.hit.position.unwrap().xy() - offset;

        dbg!(hit);
        let cursor = text.buffer().hit(hit.x, hit.y).unwrap();
        dbg!(cursor);
        edit.byte_cursor = cursor.index;

        let (mut node, mut cursor) = q3.get_mut(children[1]).unwrap();
        node.display = Display::Flex;
        cursor.blink.reset();
    }
}

fn input(
    trigger: Trigger<FocusedInput<KeyboardInput>>,
    input: Res<ButtonInput<KeyCode>>,
    children: Query<&Children>,
    mut q: Query<(&mut Text, &mut TextEdit)>,
    mut input_focus: ResMut<InputFocus>,
    mut cursor: Query<(&mut Node, &mut TextEditCursor)>,
) {
    let _shift = input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let control = input.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);

    if trigger.event().input.state == ButtonState::Released {
        return;
    }

    let children = children.get(trigger.target()).unwrap();
    let text_child = children[0];
    let cursor_child = children[1];

    let (mut node, mut cursor) = cursor.get_mut(cursor_child).unwrap();
    node.display = Display::Flex;
    cursor.blink.reset();

    let (mut text, mut text_edit) = q.get_mut(text_child).unwrap();

    if let Some(input_text) = &trigger.event().input.text {
        let filtered_text = input_text
            .chars()
            .filter(|c| !c.is_control())
            .collect::<String>();
        let idx = text_edit.byte_cursor;
        text_edit.text.insert_str(idx, &filtered_text);
        text_edit.byte_cursor += filtered_text.len();
    }

    match trigger.event().input.logical_key {
        Key::Tab => {}
        Key::ArrowDown => {}
        Key::ArrowLeft if text_edit.byte_cursor > 0 => {
            for _ in 0..4 {
                text_edit.byte_cursor -= 1;
                if text_edit.text.is_char_boundary(text_edit.byte_cursor) {
                    break;
                }
            }
        }
        Key::ArrowRight if text_edit.byte_cursor < text_edit.text.len() => {
            for _ in 0..4 {
                text_edit.byte_cursor += 1;
                if text_edit.text.is_char_boundary(text_edit.byte_cursor) {
                    break;
                }
            }
        }
        Key::ArrowUp => {}
        Key::End => {}
        Key::Home => {}
        Key::PageDown => {}
        Key::PageUp => {}
        Key::Backspace if control && text_edit.byte_cursor > 0 => {
            // Repeat backspace action while we are on the same class of character
            let mut is_removing_whitespace = true;
            let mut char_class = None;
            while text_edit.byte_cursor > 0 {
                // Move left
                let prev = text_edit.byte_cursor;
                for _ in 0..4 {
                    text_edit.byte_cursor -= 1;
                    if text_edit.text.is_char_boundary(text_edit.byte_cursor) {
                        break;
                    }
                }

                let char = text_edit.text[text_edit.byte_cursor..]
                    .chars()
                    .next()
                    .unwrap();

                // Check if this character is whitespace
                if char.is_whitespace() {
                    if is_removing_whitespace {
                        // we remove, continue
                        let idx = text_edit.byte_cursor;
                        text_edit.text.remove(idx);
                        continue;
                    } else {
                        // we stop
                        text_edit.byte_cursor = prev;
                        break;
                    }
                }
                is_removing_whitespace = false;
                // Check if this is the same class as previous (if any)
                let cur_class = Some(char.is_alphanumeric());
                if char_class.is_none() || char_class == cur_class {
                    char_class = cur_class;
                    // we remove, continue
                    let idx = text_edit.byte_cursor;
                    text_edit.text.remove(idx);
                    continue;
                }

                // we stop
                text_edit.byte_cursor = prev;
                break;
            }
        }
        Key::Backspace if text_edit.byte_cursor > 0 => {
            // Move cursor left, then delete
            for _ in 0..4 {
                text_edit.byte_cursor -= 1;
                if text_edit.text.is_char_boundary(text_edit.byte_cursor) {
                    break;
                }
            }
            let idx = text_edit.byte_cursor;
            text_edit.text.remove(idx);
        }
        Key::Delete if control && text_edit.byte_cursor < text_edit.text.len() => {
            // Repeat delete action while we are on the same class of character
            let mut is_removing_whitespace = true;
            let mut char_class = None;
            while text_edit.byte_cursor < text_edit.text.len() {
                let char = text_edit.text[text_edit.byte_cursor..]
                    .chars()
                    .next()
                    .unwrap();

                // Check if this character is whitespace
                if char.is_whitespace() {
                    if is_removing_whitespace {
                        // we remove, continue
                        let idx = text_edit.byte_cursor;
                        text_edit.text.remove(idx);
                        continue;
                    } else {
                        // we stop
                        break;
                    }
                }
                is_removing_whitespace = false;
                // Check if this is the same class as previous (if any)
                let cur_class = Some(char.is_alphanumeric());
                if char_class.is_none() || char_class == cur_class {
                    char_class = cur_class;
                    // we remove, continue
                    let idx = text_edit.byte_cursor;
                    text_edit.text.remove(idx);
                    continue;
                }

                // we stop
                break;
            }
        }
        Key::Escape => {
            // unfocus
            input_focus.0 = None;
        }
        _ => {}
    }

    if text_edit.is_changed() {
        if text_edit.text.is_empty() {
            text.0 = text_edit.placeholder.clone();
        } else {
            text.0 = text_edit.text.clone();
        }
    }
}

fn update_cursor(
    q: Query<
        (&ComputedTextBlock, &TextEdit, &ChildOf),
        Or<(Changed<ComputedTextBlock>, Changed<TextEdit>)>,
    >,
    q2: Query<&Children>,
    mut q3: Query<&mut Node>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    for (block, text_edit, child_of) in q {
        let cursor_e = q2.get(child_of.parent()).unwrap()[1];
        let mut cursor = q3.get_mut(cursor_e).unwrap();

        let buffer = block.buffer();
        // Only support single line
        assert_eq!(buffer.lines.len(), 1);
        let line = &buffer.lines[0];
        let cache = line.layout_opt().unwrap();
        let cursor_x = if text_edit.byte_cursor == text_edit.text.len() {
            cache
                .last()
                .map(|line| line.glyphs.last())
                .flatten()
                .map(|g| g.x + g.w)
        } else {
            cache
                .iter()
                .map(|l| l.glyphs.iter())
                .flatten()
                .find(|g| g.start == text_edit.byte_cursor)
                .map(|g| g.x)
        }
        .unwrap_or(0.0);

        let scale = window.scale_factor();
        cursor.left = Val::Px(cursor_x / scale);
    }
}

fn cursor_blink(
    cursor: Query<(&mut Node, &mut TextEditCursor, &ChildOf)>,
    is_focused: IsFocusedHelper,
    time: Res<Time>,
) {
    for (mut node, mut cursor, child_of) in cursor {
        if is_focused.is_focused(child_of.parent()) {
            if !cursor.focused {
                cursor.focused = true;
                node.display = Display::Flex;
            }

            if cursor.blink.tick(time.delta()).finished() {
                node.display = match node.display {
                    Display::Flex => Display::None,
                    Display::None => Display::Flex,
                    _ => unreachable!(),
                };
            }
        } else {
            if cursor.focused {
                cursor.focused = false;
                node.display = Display::None;
            }
        }
    }
}
