pub use once_cell::sync::OnceCell;
pub use thiserror::Error;
pub use tokio::sync::Mutex;
pub use uuid::Uuid;
pub use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Error)]
pub enum PerformError {
    #[error("NotInitialized")]
    NotInitialized,
    #[error("Locked")]
    Locked,
    #[error("Empty")]
    Empty,
}

#[macro_export]
macro_rules! build_perform {
    ($space:ident, $value:ty) => {
        mod $space {
            use std::collections::HashMap;
            use std::future::Future;
            use std::hash::Hash;
            type V = $value;
            type H = HashMap<$crate::Uuid, Result<V, $crate::PerformError>>;

            static STORE: $crate::OnceCell<$crate::Mutex<H>> = $crate::OnceCell::new();

            fn global_data() -> &'static $crate::Mutex<H>
            where
                $crate::Uuid: Hash,
            {
                STORE.get_or_init(|| {
                    let hash_map = HashMap::new();
                    $crate::Mutex::new(hash_map)
                })
            }

            async fn lock_and_do_mut<F, R>(f: F) -> R
            where
                F: FnOnce(&mut H) -> R,
                $crate::Uuid: Hash,
            {
                let mut hash_map = global_data().lock().await;
                f(&mut *hash_map)
            }
            async fn lock_and_do<F, R>(f: F) -> R
            where
                F: Fn(&H) -> R,
            {
                let hash_map = global_data().lock().await;
                f(&*hash_map)
            }

            #[allow(dead_code)]
            fn try_lock_and_do_mut<F, R>(f: F) -> Option<R>
            where
                F: FnOnce(&mut H) -> R,
            {
                let mut hash_map = global_data().try_lock().ok()?;
                Some(f(&mut *hash_map))
            }
            #[allow(dead_code)]
            fn try_lock_and_do<F, R>(f: F) -> Option<R>
            where
                F: Fn(&H) -> R,
            {
                let hash_map = global_data().try_lock().ok()?;
                Some(f(&*hash_map))
            }

            pub async fn activate() -> $crate::Uuid {
                let id = $crate::Uuid::new_v4();
                lock_and_do_mut(|hash_map| hash_map.insert(id, Err($crate::PerformError::Empty)))
                    .await;
                id
            }
            pub fn activate_with_spawn_local() -> $crate::Uuid {
                let id = $crate::Uuid::new_v4();
                $crate::spawn_local(async move {
                    lock_and_do_mut(|hash_map| {
                        hash_map.insert(id, Err($crate::PerformError::Empty));
                    })
                    .await;
                });
                id
            }

            pub async fn perform<Fut>(id: $crate::Uuid, fut: Fut)
            where
                Fut: Future<Output = $value> + 'static,
            {
                let value = fut.await;
                lock_and_do_mut(|hash_map| hash_map.insert(id, Ok(value))).await;
            }
            #[allow(dead_code)]
            pub fn perform_with_spawn_local<Fut>(id: $crate::Uuid, fut: Fut)
            where
                Fut: Future<Output = $value> + 'static,
            {
                $crate::spawn_local(async move {
                    let value = fut.await;
                    lock_and_do_mut(|hash_map| hash_map.insert(id, Ok(value))).await;
                });
            }

            pub async fn take(id: $crate::Uuid) -> Result<V, $crate::PerformError> {
                lock_and_do_mut(|hash_map| {
                    let some_result: Option<Result<V, $crate::PerformError>> =
                        hash_map.remove_entry(&id).map(|(_id, r)| r);
                    match some_result {
                        Some(result) => result.map_err(|_| $crate::PerformError::Empty),
                        None => Err($crate::PerformError::NotInitialized),
                    }
                })
                .await
            }

            #[allow(dead_code)]
            pub fn try_take(id: $crate::Uuid) -> Result<V, $crate::PerformError> {
                let optional_result = try_lock_and_do_mut(|hash_map| {
                    let some_result: Option<Result<V, $crate::PerformError>> =
                        hash_map.remove_entry(&id).map(|(_id, r)| r);
                    match some_result {
                        Some(result) => result.map_err(|_| $crate::PerformError::Empty),
                        None => Err($crate::PerformError::NotInitialized),
                    }
                });
                match optional_result {
                    Some(result) => result,
                    None => Err($crate::PerformError::Locked),
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use wasm_bindgen_test::wasm_bindgen_test;
    use wasm_bindgen_test::wasm_bindgen_test_configure;
    wasm_bindgen_test_configure!(run_in_browser);

    build_perform!(test_module, String);
    use test_module::*;

    #[test]
    fn first_test() {
        assert!(true);
    }

    #[wasm_bindgen_test]
    async fn second_test() -> anyhow::Result<()> {
        console_error_panic_hook::set_once();
        wasm_logger::init(wasm_logger::Config::default());
        log::trace!("some trace log");
        log::debug!("some debug log");
        log::info!("some info log");
        log::warn!("some warn log");
        log::error!("some error log");

        let fut = async {
            reqwest::get("http://httpbin.org/ip")
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        };
        let id = test_module::activate().await;
        test_module::perform(id, fut).await;
        let ip_result = test_module::take(id).await;
        assert!(ip_result.is_ok());
        let ip = ip_result?;
        assert!(ip.contains("origin"));
        let ip_result = test_module::take(id).await;
        assert!(ip_result.is_err());
        Ok(())
    }
}
