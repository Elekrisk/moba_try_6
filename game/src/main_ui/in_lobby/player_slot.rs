use bevy::{
    ecs::{component::HookContext, query::QueryFilter, spawn::SpawnIter, world::DeferredWorld},
    platform::collections::HashMap,
    prelude::*,
};
use lobby_common::{ClientToLobby, PlayerId, PlayerInfo, Team};

use crate::{
    LobbySender,
    main_ui::send_msg,
    ui::{GlobalObserver, ObservedBy, button::button2},
};

use super::{CurrentLobbyInfo, PlayerInfoCache};

pub fn client(app: &mut App) {
    app.add_observer(update_known_lobby_player_names)
        .insert_resource(PlayerSlotMap {
            pos_to_entity: HashMap::new(),
        });
}

#[derive(Resource)]
pub struct PlayerSlotMap {
    pos_to_entity: HashMap<(Team, usize), Entity>,
}

impl PlayerSlotMap {
    pub fn get(&self, team: Team, index: usize) -> Option<Entity> {
        self.pos_to_entity.get(&(team, index)).copied()
    }
}

#[derive(Clone, Copy, Component)]
#[component(immutable)]
#[component(on_insert = slot_inserted)]
#[component(on_remove = slot_removed)]
#[component(on_replace = slot_removed)]
pub struct PlayerSlot(pub Team, pub usize);

fn slot_inserted(mut world: DeferredWorld, ctx: HookContext) {
    let pos = *world.entity(ctx.entity).get::<PlayerSlot>().unwrap();
    let mut map = world.resource_mut::<PlayerSlotMap>();
    map.pos_to_entity.insert((pos.0, pos.1), ctx.entity);
}

fn slot_removed(mut world: DeferredWorld, ctx: HookContext) {
    let pos = *world.entity(ctx.entity).get::<PlayerSlot>().unwrap();
    let mut map = world.resource_mut::<PlayerSlotMap>();
    map.pos_to_entity.remove(&(pos.0, pos.1));
}

#[derive(Event)]
pub struct PlayerMovedToThisSlot(pub PlayerId);
#[derive(Event)]
pub struct PlayerMovedFromThisSlot;

pub fn player_slot(player: Option<PlayerId>, team: Team, index: usize) -> impl Bundle {
    (
        Node {
            height: Val::Px(40.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BorderColor(Color::WHITE),
        Children::spawn(SpawnIter(player.map(player_slot_content).into_iter())),
        PlayerSlot(team, index),
        ObservedBy::new(
            |trigger: Trigger<PlayerMovedToThisSlot>, mut commands: Commands| {
                commands
                    .entity(trigger.target())
                    .despawn_related::<Children>()
                    .with_child(player_slot_content(trigger.event().0));
            },
        ),
        ObservedBy::new(
            |trigger: Trigger<PlayerMovedFromThisSlot>, mut commands: Commands| {
                commands
                    .entity(trigger.target())
                    .despawn_related::<Children>();
            },
        ),
        ObservedBy::new(
            |trigger: Trigger<Pointer<DragDrop>>,
             slot_q: Query<&PlayerSlot>,
             content_q: Query<&PlayerSlotContents>,
             parent: Query<&ChildOf>,
             children: Query<&Children>,
             sender: Res<LobbySender>| {
                info!("Dropped: {}", trigger.event().event.dropped);
                let Ok(q) = content_q.get(trigger.event().event.dropped) else {
                    return;
                };
                let slot = slot_q.get(trigger.target()).unwrap();
                let orig_slot_e = parent.get(trigger.event().event.dropped).unwrap().parent();
                let orig_slot = slot_q.get(orig_slot_e).unwrap();

                if let Ok(children) = children.get(trigger.target())
                    && children.len() > 0
                {
                    let other = content_q.get(children[0]).unwrap();
                    _ = sender.send(ClientToLobby::SwitchPlayerPositions(q.0, other.0));
                } else if orig_slot.0 != slot.0 {
                    _ = sender.send(ClientToLobby::ChangePlayerTeam(q.0, slot.0));
                }
            },
        ),
    )
}

#[derive(Component)]
#[component(immutable)]
pub struct PlayerSlotContents(PlayerId);

#[derive(Component)]
pub struct PlayerNameText;
#[derive(Component)]
pub struct LeaderText;

fn player_slot_content(player: PlayerId) -> impl Bundle {
    (
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.35, 0.35, 0.35)),
        PlayerSlotContents(player),
        ObservedBy::new(
            |trigger: Trigger<Pointer<DragStart>>, mut commands: Commands| {
                commands
                    .entity(trigger.target())
                    .insert(Pickable::IGNORE);
                commands.entity(trigger.target()).insert(GlobalZIndex(1));
            },
        ),
        ObservedBy::new(
            |trigger: Trigger<Pointer<Drag>>, mut node: Query<&mut Node>| {
                let mut node = node.get_mut(trigger.target()).unwrap();
                let distance = trigger.event().distance;
                node.left = Val::Px(distance.x);
                node.top = Val::Px(distance.y);
            },
        ),
        ObservedBy::new(
            |trigger: Trigger<Pointer<DragEnd>>,
             mut node: Query<&mut Node>,
             mut commands: Commands| {
                let mut node = node.get_mut(trigger.target()).unwrap();
                node.left = Val::Auto;
                node.top = Val::Auto;
                commands
                    .entity(trigger.target())
                    .remove::<Pickable>();
                commands.entity(trigger.target()).remove::<GlobalZIndex>();
            },
        ),
        children![
            (LeaderText, Text::new(""), Pickable::IGNORE),
            (
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
                PlayerNameText,
                Text::new("Unknown"),
                Pickable::IGNORE
            ),
            button2("Kick", send_msg(ClientToLobby::KickPlayer(player))),
        ],
    )
}

pub fn update_lobby_player_names(
    info: In<PlayerInfo>,
    current_lobby: Res<CurrentLobbyInfo>,
    q: Query<(&Children, &PlayerSlotContents)>,
    lq: Query<&mut Text, With<LeaderText>>,
    tq: Query<&mut Text, (With<PlayerNameText>, Without<LeaderText>)>,
) {
    for (children, slot) in &q {
        if info.id == slot.0 {
            set_text(children, tq, info.0.name);
            let is_leader = current_lobby.0.leader == info.0.id;
            set_text(children, lq, if is_leader { "[L] " } else { "" });
            break;
        }
    }
}

fn set_text(
    children: &Children,
    mut q: Query<&mut Text, impl QueryFilter>,
    text: impl Into<String>,
) {
    for e in children {
        if let Ok(mut txt) = q.get_mut(*e) {
            txt.0 = text.into();
            info!("MONEY CACHE");
            break;
        } else {
            info!("Child doesn't match");
        }
    }
}

fn update_known_lobby_player_names(
    trigger: Trigger<OnInsert, PlayerNameText>,
    cache: Res<PlayerInfoCache>,
    current_lobby: Res<CurrentLobbyInfo>,
    parent: Query<&ChildOf>,
    q: Query<(&PlayerSlotContents, &Children)>,
    lq: Query<&mut Text, With<LeaderText>>,
    tq: Query<&mut Text, (With<PlayerNameText>, Without<LeaderText>)>,
    sender: Res<LobbySender>,
) {
    let e = parent.get(trigger.target()).unwrap();
    let (slot, children) = q.get(e.parent()).unwrap();
    if let Some(info) = cache.cache.get(&slot.0) {
        info!("CACHE MONEY: {}", children.len());
        set_text(children, tq, &info.name);
        let is_leader = current_lobby.0.leader == info.id;
        set_text(children, lq, if is_leader { "[L] " } else { "" });
    } else {
        let _ = sender.0.send(ClientToLobby::GetPlayerInfo(slot.0));
    }
}

// fn update_known_lobby_player_names(
//     trigger: Trigger<OnInsert, PlayerSlotContents>,
//     cache: Res<PlayerInfoCache>,
//     current_lobby: Res<CurrentLobbyInfo>,
//     q: Query<(&PlayerSlotContents, &Children)>,
//     lq: Query<&mut Text, With<LeaderText>>,
//     tq: Query<&mut Text, (With<PlayerNameText>, Without<LeaderText>)>,
//     sender: Res<LobbySender>,
// ) {
//     let (slot, children) = q.get(trigger.target()).unwrap();
//     if let Some(info) = cache.cache.get(&slot.0) {
//         info!("CACHE MONEY: {}", children.len());
//         set_text(children, tq, &info.name);
//         let is_leader = current_lobby.0.leader == info.id;
//         set_text(children, lq, if is_leader { "[L] " } else { "" });
//     } else {
//         let _ = sender.0.send(ClientToLobby::GetPlayerInfo(slot.0));
//     }
// }
