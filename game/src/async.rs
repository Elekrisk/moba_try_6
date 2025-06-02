use std::sync::Arc;

use bevy::prelude::*;
use tokio::runtime::Runtime;

#[cfg(target_family = "wasm")]
pub fn client(app: &mut App) {
    app.insert_resource(AsyncContext {});
}

#[derive(Resource, Clone)]
pub struct AsyncContext {
    #[cfg(not(target_family = "wasm"))]
    runtime: Arc<Runtime>,
}

impl AsyncContext {
    #[cfg(target_family = "wasm")]
    pub fn run<F>(&self, future: F)
    where
        F: Future<Output = ()> + 'static,
    {
        wasm_bindgen_futures::spawn_local(async move {
            let local = tokio::task::LocalSet::new();
            local
                .run_until(async move {
                    tokio::task::spawn_local(future).await.unwrap();
                })
                .await;
        });
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn run<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.runtime.spawn(future);
    }

    // pub fn run_ignore<F, T>(&self, future: F)
    // where
    //     F: Future<Output = T> + Send + 'static,
    // {
    //     self.run(async {
    //         let _ = future.await;
    //     });
    // }
}

#[cfg(not(target_family = "wasm"))]
pub fn common(app: &mut App) {
    app.insert_resource(AsyncContext {
        runtime: Arc::new(Runtime::new().unwrap()),
    });
}
