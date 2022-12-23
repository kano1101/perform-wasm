pub use once_cell::sync::OnceCell;
pub use thiserror::Error;
pub use tokio::sync::Mutex;
pub use uuid::Uuid;
pub use wasm_bindgen_futures::spawn_local;

pub struct Session {
    id: Uuid,
}

#[derive(Debug, Error, Clone)]
pub enum PerformError {
    #[error("NotSecured")]
    NotSecured,
    #[error("Locked")]
    Locked,
}

#[derive(Clone)]
pub enum PerformState<T> {
    Empty,
    Done(T),
}

#[macro_export]
macro_rules! build_perform {
    ($space:ident, $value:ty) => {
        mod $space {
            use std::collections::HashMap;
            use std::future::Future;
            type V = $crate::PerformState<$value>;
            type H = HashMap<$crate::Uuid, Result<V, $crate::PerformError>>;

            static STORE: $crate::OnceCell<$crate::Mutex<H>> = $crate::OnceCell::new();

            fn global_data() -> &'static $crate::Mutex<H> {
                STORE.get_or_init(|| {
                    let hash_map = HashMap::new();
                    $crate::Mutex::new(hash_map)
                })
            }

            async fn lock_and_do_mut<F, R>(f: F) -> R
            where
                F: FnOnce(&mut H) -> R,
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
            fn try_lock_and_do_mut<F>(f: F) -> Result<V, $crate::PerformError>
            where
                F: FnOnce(&mut H) -> Result<V, $crate::PerformError>,
            {
                let try_lock = global_data().try_lock();
                match try_lock {
                    Ok(mut hash_map) => f(&mut *hash_map),
                    Err(_) => Err($crate::PerformError::Locked),
                }
            }
            #[allow(dead_code)]
            fn try_lock_and_do<F>(f: F) -> Result<V, $crate::PerformError>
            where
                F: Fn(&H) -> Result<V, $crate::PerformError>,
            {
                let try_lock = global_data().try_lock();
                match try_lock {
                    Ok(hash_map) => f(&*hash_map),
                    Err(_) => Err($crate::PerformError::Locked),
                }
            }

            impl $crate::Session {
                pub async fn activate() -> Self {
                    let id = $crate::Uuid::new_v4();
                    lock_and_do_mut(|hash_map| {
                        hash_map.insert(id, Ok($crate::PerformState::Empty))
                    })
                    .await;
                    Self { id }
                }
                pub fn activate_with_spawn_local() -> Self {
                    let id = $crate::Uuid::new_v4();
                    $crate::spawn_local(async move {
                        lock_and_do_mut(|hash_map| {
                            hash_map.insert(id, Ok($crate::PerformState::Empty));
                        })
                        .await;
                    });
                    Self { id }
                }

                pub async fn perform<Fut>(&self, fut: Fut)
                where
                    Fut: Future<Output = $value> + 'static,
                {
                    let value = fut.await;
                    lock_and_do_mut(|hash_map| {
                        hash_map.insert(self.id, Ok($crate::PerformState::Done(value)))
                    })
                    .await;
                }
                #[allow(dead_code)]
                pub fn perform_with_spawn_local<Fut>(&self, fut: Fut)
                where
                    Fut: Future<Output = $value> + 'static,
                {
                    let id = self.id.clone();
                    $crate::spawn_local(async move {
                        let value = fut.await;
                        lock_and_do_mut(|hash_map| {
                            hash_map.insert(id, Ok($crate::PerformState::Done(value)))
                        })
                        .await;
                    });
                }

                fn get_as_clone<'a>(
                    id: &$crate::Uuid,
                    hash_map: &'a H,
                ) -> Option<&'a Result<$crate::PerformState<String>, $crate::PerformError>>
                {
                    hash_map.get(id)
                }
                fn get_as_take(
                    hash_map: &mut H,
                    id: &$crate::Uuid,
                ) -> Option<Result<$crate::PerformState<String>, $crate::PerformError>> {
                    hash_map.remove_entry(id).map(|(_id, r)| r)
                }
                fn into_as_clone<T, E>(result: &Result<T, E>) -> Result<T, E>
                where
                    T: Clone,
                    E: Clone,
                {
                    match result {
                        Ok(v) => Ok(v.clone()),
                        Err(e) => Err(e.clone()),
                    }
                }
                fn into_as_take<T, E>(result: Result<T, E>) -> Result<T, E> {
                    result
                }

                fn clone_from_id(
                    hash_map: &H,
                    id: &$crate::Uuid,
                ) -> Result<V, $crate::PerformError> {
                    let some_result = Self::get_as_clone(id, hash_map);
                    match some_result {
                        Some(result) => Self::into_as_clone(result),
                        None => Err($crate::PerformError::NotSecured),
                    }
                }
                fn take_from_id(
                    hash_map: &mut H,
                    id: &$crate::Uuid,
                ) -> Result<V, $crate::PerformError> {
                    let some_result = Self::get_as_take(hash_map, id);
                    match some_result {
                        Some(result) => Self::into_as_take(result),
                        None => Err($crate::PerformError::NotSecured),
                    }
                }

                pub async fn clone(&self) -> Result<V, $crate::PerformError> {
                    lock_and_do(|hash_map| Self::clone_from_id(hash_map, &self.id)).await
                }
                pub async fn take(&self) -> Result<V, $crate::PerformError> {
                    lock_and_do_mut(|hash_map| Self::take_from_id(hash_map, &self.id)).await
                }

                #[allow(dead_code)]
                pub fn try_clone(&self) -> Result<V, $crate::PerformError> {
                    try_lock_and_do(|hash_map| Self::clone_from_id(hash_map, &self.id))
                }
                #[allow(dead_code)]
                pub fn try_take(&self) -> Result<V, $crate::PerformError> {
                    try_lock_and_do_mut(|hash_map| Self::take_from_id(hash_map, &self.id))
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
    use crate::{PerformState, Session};

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

        let session = Session::activate().await;
        session.perform(fut).await;

        let ip_result = session.take().await;
        assert!(ip_result.is_ok());

        if let PerformState::Done(ip) = ip_result? {
            assert!(ip.contains("origin"));
        } else {
            assert!(false);
        }

        let ip_result = session.take().await;
        assert!(ip_result.is_err());
        log::debug!("成功しました。");

        Ok(())
    }
}
