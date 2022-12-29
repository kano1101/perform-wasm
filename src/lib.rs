pub use async_trait::async_trait;
pub use once_cell::sync::OnceCell;
pub use thiserror::Error;
pub use tokio::sync::Mutex;
pub use uuid::Uuid;

#[allow(unused_imports)]
pub use wasm_bindgen_futures::spawn_local;

use std::collections::HashMap;

#[async_trait]
pub trait Performer<T> {
    async fn activate() -> Self;
    #[allow(dead_code)]
    fn activate_with_spawn_local() -> Self;

    async fn perform<Fut>(&self, fut: Fut)
    where
        Fut: std::future::Future<Output = T> + 'static + Send;
    #[allow(dead_code)]
    fn perform_with_spawn_local<Fut>(&self, fut: Fut)
    where
        Fut: std::future::Future<Output = T> + 'static;

    async fn take(&self) -> Result<PerformState<T>, PerformError>;

    fn try_take(&self) -> Result<PerformState<T>, PerformError>;

    fn take_from_id(
        &self,
        hash_map: &mut HashMap<Uuid, Result<PerformState<T>, PerformError>>,
        id: &Uuid,
    ) -> Result<PerformState<T>, PerformError>;
    fn get_as_take(
        &self,
        hash_map: &mut HashMap<Uuid, Result<PerformState<T>, PerformError>>,
        id: &Uuid,
    ) -> Option<Result<PerformState<T>, PerformError>>;
    fn into_as_take<U, E>(&self, result: Result<U, E>) -> Result<U, E>;
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

#[derive(PartialEq)]
pub enum Progress {
    Triggered,
    Off,
}

#[macro_export]
macro_rules! build_perform {
    ($value:ty) => {
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

        pub struct Session {
            #[allow(dead_code)]
            id: $crate::Uuid,
        }

        #[$crate::async_trait]
        impl $crate::Performer<$value> for Session {
            async fn activate() -> Self {
                let id = $crate::Uuid::new_v4();
                lock_and_do_mut(|hash_map| hash_map.insert(id, Ok($crate::PerformState::Empty)))
                    .await;
                Self { id }
            }
            #[allow(dead_code)]
            fn activate_with_spawn_local() -> Self {
                let id = $crate::Uuid::new_v4();
                $crate::spawn_local(async move {
                    lock_and_do_mut(|hash_map| {
                        hash_map.insert(id, Ok($crate::PerformState::Empty));
                    })
                    .await;
                });
                Self { id }
            }

            async fn perform<Fut>(&self, fut: Fut)
            where
                Fut: Future<Output = $value> + 'static + Send,
            {
                let value = fut.await;
                lock_and_do_mut(|hash_map| {
                    hash_map.insert(self.id, Ok($crate::PerformState::Done(value)))
                })
                .await;
            }
            #[allow(dead_code)]
            fn perform_with_spawn_local<Fut>(&self, fut: Fut)
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

            async fn take(&self) -> Result<V, $crate::PerformError> {
                lock_and_do_mut(|hash_map| self.take_from_id(hash_map, &self.id)).await
            }

            fn try_take(&self) -> Result<V, $crate::PerformError> {
                try_lock_and_do_mut(|hash_map| self.take_from_id(hash_map, &self.id))
            }

            fn take_from_id(
                &self,
                hash_map: &mut H,
                id: &$crate::Uuid,
            ) -> Result<V, $crate::PerformError> {
                let some_result = self.get_as_take(hash_map, id);
                match some_result {
                    Some(result) => self.into_as_take(result),
                    None => Err($crate::PerformError::NotSecured),
                }
            }
            fn get_as_take(
                &self,
                hash_map: &mut H,
                id: &$crate::Uuid,
            ) -> Option<Result<V, $crate::PerformError>> {
                hash_map.remove_entry(id).map(|(_id, r)| r)
            }
            fn into_as_take<T, E>(&self, result: Result<T, E>) -> Result<T, E> {
                result
            }
        }

        #[allow(dead_code)]
        pub struct Taker {
            session: Session,
            progress: $crate::Progress,
        }
        impl Taker {
            #[allow(dead_code)]
            pub fn new(session: Session) -> Self {
                let instance = Self {
                    session: session,
                    progress: $crate::Progress::Off,
                };
                return instance;
            }
            #[allow(dead_code)]
            pub fn try_take<F>(&mut self, fut: F) -> Option<$value>
            where
                F: std::future::Future<Output = $value> + 'static,
            {
                use $crate::Performer as _;
                let took = self.session.try_take();
                if let Ok($crate::PerformState::Done(output)) = took {
                    self.progress = $crate::Progress::Off;
                    return Some(output);
                } else {
                    if self.progress == $crate::Progress::Off {
                        self.session.perform_with_spawn_local(fut);
                        self.progress = $crate::Progress::Triggered;
                    }
                }
                return None;
            }
        }
    };
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use crate::{PerformError, PerformState, Performer};

    #[allow(dead_code)]
    async fn run_test<Fut, T, A, S>(fut: Fut, assert: A, session: S) -> anyhow::Result<()>
    where
        Fut: std::future::Future<Output = T> + 'static + Send,
        A: FnOnce(T),
        S: Performer<T>,
    {
        let session = session;
        session.perform(fut).await;

        let value_result = session.take().await;
        assert!(value_result.is_ok());

        if let PerformState::Done(value) = value_result? {
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
