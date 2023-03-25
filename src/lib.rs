pub use async_trait::async_trait;
pub use once_cell::sync::OnceCell;
pub use thiserror::Error;
pub use tokio::sync::Mutex;
pub use uuid::Uuid;

#[allow(unused_imports)]
pub use wasm_bindgen_futures::spawn_local;

use std::collections::HashMap;

#[async_trait]
pub trait Perform<T> {
    #[allow(dead_code)]
    fn try_activate() -> Self;
    async fn activate() -> Self;

    #[allow(dead_code)]
    fn perform_with_spawn_local<Fut>(&self, fut: Fut)
    where
        Fut: std::future::Future<Output = T> + 'static;
    async fn perform<Fut>(&self, fut: Fut)
    where
        Fut: std::future::Future<Output = T> + 'static + Send;

    fn try_ready(&self) -> Result<T, PerformError>;

    fn try_take(&self) -> Result<T, PerformError>;
    async fn take(&self) -> Result<T, PerformError>;

    fn take_from_id(
        &self,
        hash_map: &mut HashMap<Uuid, Result<T, PerformError>>,
        id: &Uuid,
    ) -> Result<T, PerformError>;
    fn get_as_take(
        &self,
        hash_map: &mut HashMap<Uuid, Result<T, PerformError>>,
        id: &Uuid,
    ) -> Option<Result<T, PerformError>>;
}

#[derive(Debug, Error, Clone)]
pub enum PerformError {
    #[error("Locked")]
    Locked,
    #[error("Empty")]
    Empty,
}

#[allow(dead_code)]
pub fn ok_or_empty<T>(option: Option<Result<T, PerformError>>) -> Result<T, PerformError> {
    match option {
        Some(result) => result,
        None => Err(PerformError::Empty),
    }
}

#[macro_export]
macro_rules! build_perform {
    ($value:ty) => {
        use std::collections::HashMap;
        use std::future::Future;
        type V = $value;
        type E = $crate::PerformError;
        type H = HashMap<$crate::Uuid, Result<V, E>>;

        static STORE: $crate::OnceCell<$crate::Mutex<H>> = $crate::OnceCell::new();

        fn global_data() -> &'static $crate::Mutex<H> {
            STORE.get_or_init(|| {
                let hash_map = HashMap::new();
                $crate::Mutex::new(hash_map)
            })
        }

        fn try_lock_and_do_mut<F>(f: F) -> Result<V, E>
        where
            F: FnOnce(&mut H) -> Result<V, E>,
        {
            let try_lock = global_data().try_lock();
            match try_lock {
                Ok(mut hash_map) => f(&mut *hash_map),
                Err(_) => Err(E::Locked),
            }
        }
        async fn lock_and_do_mut<F, R>(f: F) -> R
        where
            F: FnOnce(&mut H) -> R,
        {
            let mut hash_map = global_data().lock().await;
            f(&mut *hash_map)
        }

        pub struct Session {
            #[allow(dead_code)]
            id: $crate::Uuid,
        }

        #[$crate::async_trait]
        impl $crate::Perform<V> for Session {
            #[allow(dead_code)]
            fn try_activate() -> Self {
                let id = $crate::Uuid::new_v4();
                let _ = try_lock_and_do_mut(|hash_map| {
                    let option = hash_map.insert(id, Err(E::Empty));
                    $crate::ok_or_empty(option)
                });
                Self { id }
            }
            async fn activate() -> Self {
                let id = $crate::Uuid::new_v4();
                lock_and_do_mut(|hash_map| hash_map.insert(id, Err(E::Empty))).await;
                Self { id }
            }

            #[allow(dead_code)]
            fn perform_with_spawn_local<Fut>(&self, fut: Fut)
            where
                Fut: Future<Output = V> + 'static,
            {
                let id = self.id.clone();
                $crate::spawn_local(async move {
                    let value = fut.await;
                    lock_and_do_mut(|hash_map| hash_map.insert(id, Ok(value))).await;
                });
            }
            async fn perform<Fut>(&self, fut: Fut)
            where
                Fut: Future<Output = V> + 'static + Send,
            {
                let id = self.id.clone();
                let value = fut.await;
                lock_and_do_mut(|hash_map| hash_map.insert(id, Ok(value))).await;
            }

            fn try_ready(&self) -> Result<V, E> {
                let id = self.id.clone();
                try_lock_and_do_mut(|hash_map| {
                    let option = hash_map.insert(id, Err(E::Empty));
                    $crate::ok_or_empty(option)
                })
            }

            fn try_take(&self) -> Result<V, E> {
                try_lock_and_do_mut(|hash_map| self.take_from_id(hash_map, &self.id))
            }
            async fn take(&self) -> Result<V, E> {
                lock_and_do_mut(|hash_map| self.take_from_id(hash_map, &self.id)).await
            }

            fn take_from_id(&self, hash_map: &mut H, id: &$crate::Uuid) -> Result<V, E> {
                let option = self.get_as_take(hash_map, id);
                $crate::ok_or_empty(option)
            }
            fn get_as_take(&self, hash_map: &mut H, id: &$crate::Uuid) -> Option<Result<V, E>> {
                hash_map.remove_entry(id).map(|(_id, r)| r)
            }
        }

        #[derive(PartialEq)]
        enum Progress {
            Triggered,
            Off,
        }

        #[allow(dead_code)]
        pub struct Performer {
            session: Session,
            progress: Progress,
        }
        impl Performer {
            #[allow(dead_code)]
            pub fn new(session: Session) -> Self {
                let instance = Self {
                    session: session,
                    progress: Progress::Off,
                };
                return instance;
            }
            #[allow(dead_code)]
            pub fn try_take(&mut self) -> Result<V, E> {
                use $crate::Perform as _;

                self.session.try_take().and_then(|v| {
                    self.progress = Progress::Off;
                    Ok(v)
                })
            }
            #[allow(dead_code)]
            pub async fn perform_one_time_or_not<F>(&mut self, fut: F)
            where
                F: std::future::Future<Output = V> + 'static + Send,
            {
                use $crate::Perform as _;
                if self.progress == Progress::Off {
                    self.session.perform(fut).await;
                    self.progress = Progress::Triggered;
                }
            }
            #[allow(dead_code)]
            pub fn perform_one_time_or_not_with_spawn_local<F>(&mut self, fut: F)
            where
                F: std::future::Future<Output = V> + 'static,
            {
                use $crate::Perform as _;
                if self.progress == Progress::Off {
                    let _is_ready = self.session.try_ready();
                    self.session.perform_with_spawn_local(fut);
                    self.progress = Progress::Triggered;
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use crate::{Perform, PerformError};

    #[allow(dead_code)]
    async fn run_test<Fut, T, A, S>(fut: Fut, assert: A, session: S) -> anyhow::Result<()>
    where
        Fut: std::future::Future<Output = T> + 'static + Send,
        A: FnOnce(T),
        S: Perform<T>,
    {
        let session = session;
        session.perform(fut).await;

        let value_result = session.take().await;
        assert!(value_result.is_ok());

        if let Ok(value) = value_result {
            assert(value);
        } else {
            assert!(false);
        }

        let value_result = session.take().await;
        assert!(value_result.is_err());

        Ok(())
    }

    #[test]
    fn first_test() {
        assert!(true);
    }

    mod ip {
        build_perform!(String);
    }
    mod status {
        build_perform!(reqwest::StatusCode);
    }

    #[tokio::test]
    #[cfg(not(target_arch = "wasm32"))]
    async fn second_test_one_build() {
        let fut = async {
            reqwest::get("http://httpbin.org/ip")
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        };
        let assert = |text: String| {
            assert!(text.contains("origin"));
        };
        let session = ip::Session::activate().await;
        let _ = run_test(fut, assert, session).await;
        log::debug!("成功しました。");

        // assert!(false);
    }

    #[tokio::test]
    #[cfg(not(target_arch = "wasm32"))]
    async fn third_test_many_build() {
        let fut = async {
            reqwest::get("http://httpbin.org/ip")
                .await
                .unwrap()
                .status()
        };
        let assert = |status| {
            assert_eq!(status, reqwest::StatusCode::OK);
        };
        let session = status::Session::activate().await;
        let _ = run_test(fut, assert, session).await;
        log::debug!("成功しました。");

        // assert!(false);
    }
}
