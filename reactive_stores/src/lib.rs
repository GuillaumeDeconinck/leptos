use reactive_graph::{
    owner::{LocalStorage, Storage, StoredValue, SyncStorage},
    signal::{
        guards::{Plain, ReadGuard},
        ArcTrigger,
    },
    traits::{DefinedAt, IsDisposed, Notify, ReadUntracked, Track},
};
use rustc_hash::FxHashMap;
use std::{
    fmt::Debug,
    panic::Location,
    sync::{Arc, RwLock},
};

mod arc_field;
mod field;
mod iter;
mod patch;
mod path;
mod store_field;
mod subfield;

pub use arc_field::ArcField;
pub use field::Field;
pub use iter::*;
pub use patch::*;
use path::StorePath;
pub use store_field::StoreField;
pub use subfield::Subfield;

#[derive(Debug, Default)]
struct TriggerMap(FxHashMap<StorePath, ArcTrigger>);

impl TriggerMap {
    fn get_or_insert(&mut self, key: StorePath) -> ArcTrigger {
        if let Some(trigger) = self.0.get(&key) {
            trigger.clone()
        } else {
            let new = ArcTrigger::new();
            self.0.insert(key, new.clone());
            new
        }
    }

    #[allow(unused)]
    fn remove(&mut self, key: &StorePath) -> Option<ArcTrigger> {
        self.0.remove(key)
    }
}

pub struct ArcStore<T> {
    #[cfg(debug_assertions)]
    defined_at: &'static Location<'static>,
    pub(crate) value: Arc<RwLock<T>>,
    signals: Arc<RwLock<TriggerMap>>,
}

impl<T> ArcStore<T> {
    pub fn new(value: T) -> Self {
        Self {
            #[cfg(debug_assertions)]
            defined_at: Location::caller(),
            value: Arc::new(RwLock::new(value)),
            signals: Default::default(),
            /* inner: Arc::new(RwLock::new(SubscriberSet::new())), */
        }
    }
}

impl<T: Debug> Debug for ArcStore<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_struct("ArcStore");
        #[cfg(debug_assertions)]
        let f = f.field("defined_at", &self.defined_at);
        f.field("value", &self.value)
            .field("signals", &self.signals)
            .finish()
    }
}

impl<T> Clone for ArcStore<T> {
    fn clone(&self) -> Self {
        Self {
            #[cfg(debug_assertions)]
            defined_at: self.defined_at,
            value: Arc::clone(&self.value),
            signals: Arc::clone(&self.signals),
        }
    }
}

impl<T> DefinedAt for ArcStore<T> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        #[cfg(debug_assertions)]
        {
            Some(self.defined_at)
        }
        #[cfg(not(debug_assertions))]
        {
            None
        }
    }
}

impl<T> IsDisposed for ArcStore<T> {
    #[inline(always)]
    fn is_disposed(&self) -> bool {
        false
    }
}

impl<T> ReadUntracked for ArcStore<T>
where
    T: 'static,
{
    type Value = ReadGuard<T, Plain<T>>;

    fn try_read_untracked(&self) -> Option<Self::Value> {
        Plain::try_new(Arc::clone(&self.value)).map(ReadGuard::new)
    }
}

impl<T: 'static> Track for ArcStore<T> {
    fn track(&self) {
        self.get_trigger(Default::default()).notify();
    }
}

impl<T: 'static> Notify for ArcStore<T> {
    fn notify(&self) {
        self.get_trigger(self.path().into_iter().collect()).notify();
    }
}

pub struct Store<T, S = SyncStorage> {
    #[cfg(debug_assertions)]
    defined_at: &'static Location<'static>,
    inner: StoredValue<ArcStore<T>, S>,
}

impl<T> Store<T>
where
    T: Send + Sync + 'static,
{
    pub fn new(value: T) -> Self {
        Self {
            #[cfg(debug_assertions)]
            defined_at: Location::caller(),
            inner: StoredValue::new_with_storage(ArcStore::new(value)),
        }
    }
}

impl<T> Store<T, LocalStorage>
where
    T: 'static,
{
    pub fn new_local(value: T) -> Self {
        Self {
            #[cfg(debug_assertions)]
            defined_at: Location::caller(),
            inner: StoredValue::new_with_storage(ArcStore::new(value)),
        }
    }
}

impl<T: Debug, S> Debug for Store<T, S>
where
    S: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut f = f.debug_struct("Store");
        #[cfg(debug_assertions)]
        let f = f.field("defined_at", &self.defined_at);
        f.field("inner", &self.inner).finish()
    }
}

impl<T, S> Clone for Store<T, S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T, S> Copy for Store<T, S> {}

impl<T, S> DefinedAt for Store<T, S> {
    fn defined_at(&self) -> Option<&'static Location<'static>> {
        #[cfg(debug_assertions)]
        {
            Some(self.defined_at)
        }
        #[cfg(not(debug_assertions))]
        {
            None
        }
    }
}

impl<T, S> IsDisposed for Store<T, S>
where
    T: 'static,
{
    #[inline(always)]
    fn is_disposed(&self) -> bool {
        self.inner.is_disposed()
    }
}

impl<T, S> ReadUntracked for Store<T, S>
where
    T: 'static,
    S: Storage<ArcStore<T>>,
{
    type Value = ReadGuard<T, Plain<T>>;

    fn try_read_untracked(&self) -> Option<Self::Value> {
        self.inner
            .try_get_value()
            .map(|inner| inner.read_untracked())
    }
}

impl<T, S> Track for Store<T, S>
where
    T: 'static,
    S: Storage<ArcStore<T>>,
{
    fn track(&self) {
        if let Some(inner) = self.inner.try_get_value() {
            inner.track();
        }
    }
}

impl<T, S> Notify for Store<T, S>
where
    T: 'static,
    S: Storage<ArcStore<T>>,
{
    fn notify(&self) {
        if let Some(inner) = self.inner.try_get_value() {
            inner.notify();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{self as reactive_stores, Patch, Store, StoreFieldIterator};
    use reactive_graph::{
        effect::Effect,
        traits::{Read, ReadUntracked, Set, Update, Writeable},
    };
    use reactive_stores_macro::{Patch, Store};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    pub async fn tick() {
        tokio::time::sleep(std::time::Duration::from_micros(1)).await;
    }

    #[derive(Debug, Store, Patch)]
    struct Todos {
        user: String,
        todos: Vec<Todo>,
    }

    #[derive(Debug, Store, Patch)]
    struct Todo {
        label: String,
        completed: bool,
    }

    impl Todo {
        pub fn new(label: impl ToString) -> Self {
            Self {
                label: label.to_string(),
                completed: false,
            }
        }
    }

    fn data() -> Todos {
        Todos {
            user: "Bob".to_string(),
            todos: vec![
                Todo {
                    label: "Create reactive store".to_string(),
                    completed: true,
                },
                Todo {
                    label: "???".to_string(),
                    completed: false,
                },
                Todo {
                    label: "Profit".to_string(),
                    completed: false,
                },
            ],
        }
    }

    #[tokio::test]
    async fn mutating_field_triggers_effect() {
        _ = any_spawner::Executor::init_tokio();

        let combined_count = Arc::new(AtomicUsize::new(0));

        let store = Store::new(data());
        assert_eq!(store.read_untracked().todos.len(), 3);
        assert_eq!(store.user().read_untracked().as_str(), "Bob");
        Effect::new_sync({
            let combined_count = Arc::clone(&combined_count);
            move |prev: Option<()>| {
                if prev.is_none() {
                    println!("first run");
                } else {
                    println!("next run");
                }
                println!("{:?}", *store.user().read());
                combined_count.fetch_add(1, Ordering::Relaxed);
            }
        });
        tick().await;
        tick().await;
        store.user().set("Greg".into());
        tick().await;
        store.user().set("Carol".into());
        tick().await;
        store.user().update(|name| name.push_str("!!!"));
        tick().await;
        // the effect reads from `user`, so it should trigger every time
        assert_eq!(combined_count.load(Ordering::Relaxed), 4);

        store
            .todos()
            .write()
            .push(Todo::new("Create reactive stores"));
        tick().await;
        store.todos().write().push(Todo::new("???"));
        tick().await;
        store.todos().write().push(Todo::new("Profit!"));
        tick().await;
        // the effect doesn't read from `todos`, so the count should not have changed
        assert_eq!(combined_count.load(Ordering::Relaxed), 4);
    }

    #[tokio::test]
    async fn other_field_does_not_notify() {
        _ = any_spawner::Executor::init_tokio();

        let combined_count = Arc::new(AtomicUsize::new(0));

        let store = Store::new(data());

        Effect::new_sync({
            let combined_count = Arc::clone(&combined_count);
            move |prev: Option<()>| {
                if prev.is_none() {
                    println!("first run");
                } else {
                    println!("next run");
                }
                println!("{:?}", *store.todos().read());
                combined_count.fetch_add(1, Ordering::Relaxed);
            }
        });
        tick().await;
        tick().await;
        store.user().set("Greg".into());
        tick().await;
        store.user().set("Carol".into());
        tick().await;
        store.user().update(|name| name.push_str("!!!"));
        tick().await;
        // the effect reads from `user`, so it should trigger every time
        assert_eq!(combined_count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn iterator_tracks_the_field() {
        _ = any_spawner::Executor::init_tokio();

        let combined_count = Arc::new(AtomicUsize::new(0));

        let store = Store::new(data());

        Effect::new_sync({
            let combined_count = Arc::clone(&combined_count);
            move |prev: Option<()>| {
                if prev.is_none() {
                    println!("first run");
                } else {
                    println!("next run");
                }
                println!("{:?}", store.todos().iter().collect::<Vec<_>>());
                combined_count.store(1, Ordering::Relaxed);
            }
        });

        tick().await;
        store
            .todos()
            .write()
            .push(Todo::new("Create reactive store?"));
        tick().await;
        store.todos().write().push(Todo::new("???"));
        tick().await;
        store.todos().write().push(Todo::new("Profit!"));
        // the effect only reads from `todos`, so it should trigger only the first time
        assert_eq!(combined_count.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn patching_only_notifies_changed_field() {
        _ = any_spawner::Executor::init_tokio();

        let combined_count = Arc::new(AtomicUsize::new(0));

        let store = Store::new(Todos {
            user: "Alice".into(),
            todos: vec![],
        });

        Effect::new_sync({
            let combined_count = Arc::clone(&combined_count);
            move |prev: Option<()>| {
                if prev.is_none() {
                    println!("first run");
                } else {
                    println!("next run");
                }
                println!("{:?}", *store.todos().read());
                combined_count.fetch_add(1, Ordering::Relaxed);
            }
        });
        tick().await;
        tick().await;
        store.patch(Todos {
            user: "Bob".into(),
            todos: vec![],
        });
        tick().await;
        store.patch(Todos {
            user: "Carol".into(),
            todos: vec![],
        });
        tick().await;
        assert_eq!(combined_count.load(Ordering::Relaxed), 1);

        store.patch(Todos {
            user: "Carol".into(),
            todos: vec![Todo {
                label: "First Todo".into(),
                completed: false,
            }],
        });
        tick().await;
        assert_eq!(combined_count.load(Ordering::Relaxed), 2);
    }
}
