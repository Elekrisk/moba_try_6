use std::marker::PhantomData;

use bevy::{
    ecs::{
        bundle::{BundleEffect, DynamicBundle},
        component::{
            ComponentId, Components, ComponentsRegistrator, RequiredComponents, StorageType,
        },
        system::IntoObserverSystem,
    },
    prelude::{Event, *},
    ptr::OwningPtr,
};

pub mod button;
pub mod scrollable;
pub mod style;
pub mod tab_bar;
pub mod text;

pub macro custom_effect($name:ident $(<$($t:ident),* $(,)?> $(where $($tt:tt)*)?)?) {
    unsafe impl $(<$($t),*>)? bevy::ecs::bundle::Bundle for $name $(<$($t),*>)? $($(where $($tt)*)?)? {
        fn component_ids(
            _components: &mut bevy::ecs::component::ComponentsRegistrator,
            _ids: &mut impl FnMut(bevy::ecs::component::ComponentId),
        ) {
        }

        fn get_component_ids(
            _components: &bevy::ecs::component::Components,
            _ids: &mut impl FnMut(Option<bevy::ecs::component::ComponentId>),
        ) {
        }

        fn register_required_components(
            _components: &mut bevy::ecs::component::ComponentsRegistrator,
            _required_components: &mut bevy::ecs::component::RequiredComponents,
        ) {
        }
    }

    impl $(<$($t),*>)? bevy::ecs::bundle::DynamicBundle for $name $(<$($t),*>)? $($(where $($tt)*)?)? {
        type Effect = Self;

        fn get_components(
            self,
            _func: &mut impl FnMut(bevy::ecs::component::StorageType, bevy::ptr::OwningPtr<'_>),
        ) -> Self::Effect {
            self
        }
    }
}

pub fn client(app: &mut App) {
    app.add_plugins((
        // tab_bar::client,
        // scrollable::client,
        // text::client,
        // button::client,
        style::client,
    ));
}

pub struct ObservedBy<E, B, M, F>
where
    E: Event,
    B: Bundle,
    F: IntoObserverSystem<E, B, M>,
{
    func: F,
    _phantom: PhantomData<(E, B, M)>,
}

impl<E, B, M, F> ObservedBy<E, B, M, F>
where
    E: Event,
    B: Bundle,
    F: IntoObserverSystem<E, B, M>,
{
    pub fn new(func: F) -> Self {
        Self {
            func,
            _phantom: PhantomData,
        }
    }
}

custom_effect!(ObservedBy<E, B, M, F> where E: Event, B: Bundle, M: Send + Sync + 'static, F: IntoObserverSystem<E, B, M> + Sync);

impl<E, B, M, F> BundleEffect for ObservedBy<E, B, M, F>
where
    E: Event,
    B: Bundle,
    M: Send + Sync,
    F: IntoObserverSystem<E, B, M> + Sync,
{
    fn apply(self, entity: &mut EntityWorldMut) {
        entity.observe(self.func);
    }
}

pub struct GlobalObserver<E, B, M, F, FF>
where
    E: Event,
    B: Bundle,
    F: IntoObserverSystem<E, B, M>,
    FF: FnOnce(Entity) -> F,
{
    func_creator: FF,
    _phantom: PhantomData<(E, B, M)>,
}

impl<E, B, M, F, FF> GlobalObserver<E, B, M, F, FF>
where
    E: Event,
    B: Bundle,
    F: IntoObserverSystem<E, B, M>,
    FF: FnOnce(Entity) -> F,
{
    pub fn new(func_creator: FF) -> Self {
        Self {
            func_creator,
            _phantom: PhantomData,
        }
    }
}

custom_effect!(GlobalObserver<E, B, M, F, FF> where E: Event, B: Bundle, M: Send + Sync + 'static, F: IntoObserverSystem<E, B, M> + Sync, FF: FnOnce(Entity) -> F + Send + Sync + 'static);

impl<E, B, M, F, FF> BundleEffect for GlobalObserver<E, B, M, F, FF>
where
    E: Event,
    B: Bundle,
    M: Send + Sync,
    F: IntoObserverSystem<E, B, M> + Sync,
    FF: FnOnce(Entity) -> F,
{
    fn apply(self, entity: &mut EntityWorldMut) {
        let id = entity.id();
        entity.world_scope(|world| {
            world.add_observer((self.func_creator)(id));
        });
    }
}

pub struct CustomEffect<F: FnOnce(&mut EntityWorldMut)>(pub F);

custom_effect!(CustomEffect<F> where F: FnOnce(&mut EntityWorldMut) + Sync + Send + 'static);

impl<F: FnOnce(&mut EntityWorldMut)> BundleEffect for CustomEffect<F> {
    fn apply(self, entity: &mut EntityWorldMut) {
        self.0(entity)
    }
}
