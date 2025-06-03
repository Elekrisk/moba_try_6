use std::{
    marker::PhantomData,
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use bevy::{
    asset::{AssetLoader, AssetPath, AsyncReadExt},
    platform::collections::HashMap,
    prelude::*,
};
use mlua::prelude::*;

use super::network::ServerOptions;

#[macro_export]
macro_rules! from_lua_table {
    (struct $name:ident {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        impl mlua::FromLua for $name {
            fn from_lua(value: mlua::prelude::LuaValue, lua: &mlua::Lua) -> mlua::Result<Self> {
                let ty = value.type_name();
                let table = value.as_table().ok_or_else(|| {
                    mlua::prelude::LuaError::FromLuaConversionError {
                        from: ty,
                        to: stringify!($name).to_string(),
                        message: Some("expected table".to_string())
                    }
                })?;

                Ok($name {
                    $(
                        $item: from_lua_table!(lua table $item $([$tt])? ($item_ty))
                    ),*
                })
            }
        }
    };
    (struct W<$name:ident> {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        impl FromLua for W<$name> {
            fn from_lua(value: LuaValue, lua: &Lua) -> mlua::Result<Self> {
                let ty = value.type_name();
                let table = value.as_table().ok_or_else(|| {
                    LuaError::FromLuaConversionError {
                        from: ty,
                        to: stringify!($name).to_string(),
                        message: Some("expected table".to_string())
                    }
                })?;

                Ok(W($name {
                    $(
                        $item: from_lua_table!(lua table $item $([$tt])? ($item_ty))
                    ),*
                }))
            }
        }
    };
    ($lua:ident $table:ident $item:ident [W] ($item_ty:ty)) => {
        <crate::ingame::lua::W<$item_ty>>::from_lua($table.get(stringify!($item))?, $lua)?.0
    };
    ($lua:ident $table:ident $item:ident ($item_ty:ty)) => {
        <$item_ty>::from_lua($table.get(stringify!($item))?, $lua)?
    };
}

macro_rules! into_lua_table {
    (struct $name:ident {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        impl mlua::IntoLua for $name {
            fn into_lua(self, lua: &mlua::Lua) -> mlua::prelude::LuaResult<mlua::prelude::LuaValue> {
                let table = lua.create_table()?;
                $(
                    table.set(stringify!($item), into_lua_table!(lua self $item $([$tt])?))?;
                )*
                Ok(mlua::Value::Table(table))
            }
        }
    };
    (struct W<$name:ident> {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        impl mlua::IntoLua for W<$name> {
            fn into_lua(self, lua: &mlua::Lua) -> mlua::prelude::LuaResult<mlua::prelude::LuaValue> {
                let table = lua.create_table()?;
                $(
                    table.set(stringify!($item), into_lua_table!(lua self $item $([$tt])?))?;
                )*
                Ok(mlua::Value::Table(table))
            }
        }
    };
    ($lua:ident $self:ident $item:ident [W]) => {
        crate::ingame::lua::W($self.$item).into_lua($lua)?
    };
    ($lua:ident $self:ident $item:ident) => {
        $self.$item.into_lua($lua)?
    };
}

macro_rules! from_into_lua_table {
    (struct $name:ident {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        from_lua_table!(struct $name { $($item: $({$tt})? $item_ty),* });
        into_lua_table!(struct $name { $($item: $({$tt})? $item_ty),* });
    };
    (struct W<$name:ident> {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        from_lua_table!(struct W<$name> { $($item: $({$tt})? $item_ty),* });
        into_lua_table!(struct W<$name> { $($item: $({$tt})? $item_ty),* });
    };
}

macro_rules! proto {
    (struct $name:ident {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        from_into_lua_table!(struct $name { $($item: $({$tt})? $item_ty),* });

        impl crate::ingame::lua::Proto for $name {
            fn id(&self) -> &str {
                &self.id
            }
        }
    };
    (struct W<$name:ident> {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        from_into_lua_table!(struct W<$name> { $($item: $({$tt})? $item_ty),* });

        impl crate::ingame::lua::Proto for $name {
            fn id(&self) -> &str {
                &self.id
            }
        }
    };
}

#[derive(Resource, Deref)]
pub struct LuaCtx(pub Lua);

#[derive(Event)]
pub struct ScriptCompleted(pub Handle<LuaScript>);

struct IsServer(bool);

pub fn common(app: &mut App) {
    let lua = Lua::new();

    let is_server = IsServer(app.world().contains_resource::<ServerOptions>());
    lua.set_app_data(is_server);

    app.insert_resource(LuaCtx(lua.clone()))
        .init_resource::<ScriptInfoMap>()
        .register_asset_loader(LuaScriptLoader { lua })
        .init_asset::<LuaScript>()
        .setup_lua(setup_lua)
        ;
}

#[derive(Resource, Default, Debug, Reflect)]
pub struct ScriptInfoMap {
    map: HashMap<Handle<LuaScript>, ScriptInfo>,
}

#[derive(Debug, Default, Reflect)]
pub struct ScriptInfo {
    executed: bool,
}

fn setup_lua(_lua: &Lua) -> LuaResult<()> {

    Ok(())
}
// #[derive(Component)]
// pub struct LuaObject(pub LuaTable);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Deref, DerefMut)]
pub struct W<T>(pub T);

from_into_lua_table!(
    struct W<Vec2> {
        x: f32,
        y: f32,
    }
);

impl FromLua for W<AssetPath<'static>> {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        let string = lua
            .coerce_string(value)?
            .ok_or_else(|| LuaError::external(anyhow!("Cannot convert nil to AssetPath")))?
            .to_string_lossy();
        Ok(W(AssetPath::from_static(string)))
    }
}

impl IntoLua for W<AssetPath<'_>> {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        self.to_string().into_lua(lua)
    }
}

pub trait AppLuaExt {
    fn setup_lua(&mut self, func: impl FnOnce(&Lua) -> LuaResult<()>) -> &mut Self;
}

impl AppLuaExt for App {
    fn setup_lua(&mut self, func: impl FnOnce(&Lua) -> LuaResult<()>) -> &mut Self {
        let lua = &self.world().resource::<LuaCtx>().0;

        match func(lua) {
            Ok(()) => {}
            Err(e) => {
                error!("Error setting up lua: {e}");
                self.world_mut()
                    .send_event(AppExit::Error(2.try_into().unwrap()));
            }
        }
        self
    }
}

pub trait LuaExt {
    fn world(&self) -> mlua::AppDataRefMut<'_, World>;
    fn path<'a>(&self, path: &'a Path) -> PathBuf;
    fn path_rel<'a>(&self, path: &'a Path, rel: PathBuf) -> PathBuf;
    fn current_path(&self) -> PathBuf;
    fn set_path(&self, path: PathBuf);
    fn reset_path(&self);
    fn with_world<T>(&self, world: &mut World, func: impl FnOnce(&Self) -> T) -> T;
    fn table(&self, key: &str) -> LuaResult<LuaTable>;
    fn is_server(&self) -> bool;
    fn is_client(&self) -> bool;
}

impl LuaExt for Lua {
    fn world(&self) -> mlua::AppDataRefMut<'_, World> {
        self.app_data_mut().unwrap()
    }

    fn path<'a>(&self, path: &'a Path) -> PathBuf {
        self.path_rel(
            path,
            self.app_data_ref::<CurrentScriptPath>().unwrap().0.clone(),
        )
    }

    fn path_rel<'a>(&self, path: &'a Path, rel: PathBuf) -> PathBuf {
        if true && (path.starts_with(".") || path.starts_with("..")) {
            rel.parent().unwrap().join(path).into()
        } else {
            path.into()
        }
    }

    fn current_path(&self) -> PathBuf {
        self.app_data_ref::<CurrentScriptPath>().unwrap().0.clone()
    }

    fn set_path(&self, path: PathBuf) {
        self.set_app_data(CurrentScriptPath(path));
    }

    fn reset_path(&self) {
        self.remove_app_data::<CurrentScriptPath>();
    }

    fn with_world<T>(&self, world: &mut World, func: impl FnOnce(&Self) -> T) -> T {
        self.set_app_data(std::mem::take(world));
        let x = func(self);
        *world = self.remove_app_data().unwrap();
        x
    }

    fn table(&self, key: &str) -> LuaResult<LuaTable> {
        match self.globals().get::<LuaTable>(key) {
            Ok(table) => Ok(table),
            Err(LuaError::FromLuaConversionError { from: "nil", .. }) => {
                let table = self.create_table()?;
                self.globals().set(key, table.clone())?;
                Ok(table)
            }
            Err(e) => Err(e),
        }
    }

    fn is_server(&self) -> bool {
        self.app_data_ref::<IsServer>().unwrap().0
    }

    fn is_client(&self) -> bool {
        !self.is_server()
    }
}

pub trait AssetPathExt {
    fn relative(self, origin: &Path) -> AssetPath<'static>;
}

impl AssetPathExt for AssetPath<'static> {
    fn relative(mut self, origin: &Path) -> AssetPath<'static> {
        let path = self.path();
        if path.starts_with(".") || path.starts_with("..") {
            let parent = origin.parent().unwrap();
            let joined = parent.join(path);
            let res = AssetPath::from_static(joined);
            if let Some(label) = self.take_label() {
                res.with_label(label)
            } else {
                res
            }
        } else {
            self
        }
    }
}

// pub struct ExecuteLuaScript(pub Handle<LuaScript>, pub bool);

// impl ExecuteLuaScript {
//     pub fn new(handle: Handle<LuaScript>) -> Self {
//         Self(handle, false)
//     }

//     pub fn immediately(mut self) -> Self {
//         self.1 = true;
//         self
//     }
// }

pub struct CurrentScriptPath(pub PathBuf);

// impl Command for ExecuteLuaScript {
//     fn apply(self, world: &mut World) -> () {
//         if let Some(script) = world.resource::<Assets<LuaScript>>().get(&self.0) {
//             // let script = script.clone();
//             let lua = &world.resource::<LuaCtx>().0;

//             // world
//             //     .resource_mut::<ScriptInfoMap>()
//             //     .map
//             //     .entry(self.0.clone())
//             //     .or_default()
//             //     .executed = true;

//             // let world_val = std::mem::take(world);
//             // lua.set_app_data(world_val);
//             // lua.set_app_data(CurrentScriptPath(script.path));
//             // match script.function.call::<()>(()) {
//             //     Ok(_) => {}
//             //     Err(e) => error!("Lua error: {e}"),
//             // }
//             // *world = lua.remove_app_data().unwrap();
//             let thread_info = ThreadInfo {
//                 handle: self.0.clone(),
//                 thread: match lua.create_thread(script.function.clone()) {
//                     Ok(val) => val,
//                     Err(e) => {
//                         error!("Lua error: {e}");
//                         return;
//                     }
//                 },
//                 dependencies: vec![],
//             };
//             let mut ctx = world.resource_mut::<LuaScriptRunningContext>();
//             ctx.currently_running.push(thread_info);

//             if self.1 {
//                 // Run scripts NOW also
//                 execute_lua(world);
//             }
//         } else {
//             // error!("Script not loaded!");
//             let mut ctx = world.resource_mut::<LuaScriptRunningContext>();
//             ctx.waiting_for_load.insert(self.0.clone());
//         }
//     }
// }

#[derive(Clone, TypePath, Asset)]
pub struct LuaScript {
    pub function: LuaFunction,
    // path: PathBuf,
}

struct LuaScriptLoader {
    lua: Lua,
}

impl AssetLoader for LuaScriptLoader {
    type Asset = LuaScript;

    type Settings = ();

    type Error = anyhow::Error;

    fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext,
    ) -> impl bevy::tasks::ConditionalSendFuture<Output = std::result::Result<Self::Asset, Self::Error>>
    {
        async move {
            let mut buf = String::new();
            reader.read_to_string(&mut buf).await?;
            let script = self
                .lua
                .load(buf)
                .set_name(
                    Path::new("assets")
                        .join(load_context.path())
                        // .canonicalize()
                        // .unwrap()
                        .to_string_lossy(),
                )
                .into_function()?;
            Ok(LuaScript {
                function: script,
                // path: load_context.path().into(),
            })
        }
    }

    fn extensions(&self) -> &[&str] {
        &["lua"]
    }
}

#[derive(Resource)]
pub struct Protos<T> {
    map: HashMap<String, (LuaValue, PathBuf)>,
    lua: Lua,
    _phantom: PhantomData<T>,
}

impl<T> FromWorld for Protos<T> {
    fn from_world(world: &mut World) -> Self {
        Self {
            map: default(),
            lua: world.resource::<LuaCtx>().0.clone(),
            _phantom: default(),
        }
    }
}

impl<T: Proto> Protos<T> {
    pub fn insert(&mut self, value: LuaValue, origin: PathBuf) -> LuaResult<LuaValue> {
        let proto = T::from_lua(value.clone(), &self.lua)?;
        self.map.insert(proto.id().into(), (value.clone(), origin));
        Ok(value)
    }

    pub fn get(&self, id: &str) -> LuaResult<(T, PathBuf)> {
        let (value, origin) = self
            .map
            .get(id)
            .ok_or_else(|| LuaError::external(anyhow!("Missing proto")))?;
        let proto = T::from_lua(value.clone(), &self.lua)?;
        Ok((proto, origin.clone()))
    }
}

pub trait Proto: FromLua {
    fn id(&self) -> &str;
}
