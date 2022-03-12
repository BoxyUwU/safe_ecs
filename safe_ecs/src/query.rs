use crate::{
    dynamic_storage::ErasedBytesVec,
    errors::WorldBorrowError,
    system::Access,
    world::{Archetype, DynamicColumns, EcsTypeId, StaticColumns, Storage},
    Component, Entity, World,
};
use std::{
    any::{type_name, TypeId},
    cell,
    collections::HashMap,
    marker::PhantomData,
};

pub trait QueryParam: 'static {
    type Lock<'a>
    where
        Self: 'a;
    type LockBorrow<'a>
    where
        Self: 'a;
    type Item<'a>
    where
        Self: 'a;
    type ItemIter<'a>
    where
        Self: 'a;
    fn lock_from_world(world: &World) -> Result<Option<Self::Lock<'_>>, WorldBorrowError>;
    fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a>;
    fn archetype_matches(archetype: &Archetype, _: &HashMap<TypeId, EcsTypeId>) -> bool;
    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        lock_borrow: &mut Self::LockBorrow<'a>,
        _: &HashMap<TypeId, EcsTypeId>,
    ) -> Self::ItemIter<'a>;
    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>>;
    fn get_access() -> Result<Access, ()>;
}

impl QueryParam for () {
    type Lock<'a> = ();
    type LockBorrow<'a> = ();
    type Item<'a> = ();
    type ItemIter<'a> = std::ops::Range<usize>;

    fn lock_from_world(_: &World) -> Result<Option<Self::Lock<'_>>, WorldBorrowError> {
        Ok(Some(()))
    }

    fn lock_borrows_from_locks<'a, 'b>(_: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {
        ()
    }

    fn archetype_matches(_: &Archetype, _: &HashMap<TypeId, EcsTypeId>) -> bool {
        true
    }

    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        _: &mut Self::LockBorrow<'a>,
        _: &HashMap<TypeId, EcsTypeId>,
    ) -> Self::ItemIter<'a> {
        0..archetype.entities.len()
    }

    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>> {
        iter.next().map(|_| ())
    }

    fn get_access() -> Result<Access, ()> {
        Ok(Access::new())
    }
}

impl QueryParam for Entity {
    type Lock<'a> = ();
    type LockBorrow<'a> = ();
    type Item<'a> = Entity;
    type ItemIter<'a> = std::slice::Iter<'a, Entity>;

    fn lock_from_world(_: &World) -> Result<Option<Self::Lock<'_>>, WorldBorrowError> {
        Ok(Some(()))
    }
    fn lock_borrows_from_locks<'a, 'b>(_: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {}
    fn archetype_matches(_: &Archetype, _: &HashMap<TypeId, EcsTypeId>) -> bool {
        true
    }
    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        _: &mut Self::LockBorrow<'a>,
        _: &HashMap<TypeId, EcsTypeId>,
    ) -> Self::ItemIter<'a> {
        archetype.entities.iter()
    }
    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>> {
        iter.next().copied()
    }
    fn get_access() -> Result<Access, ()> {
        Ok(Access::new())
    }
}

impl<T: Component> QueryParam for &'static T {
    type Lock<'a> = cell::Ref<'a, StaticColumns<T>>;
    type LockBorrow<'a> = &'a [Vec<T>];
    type Item<'a> = &'a T;
    type ItemIter<'a> = std::slice::Iter<'a, T>;

    fn lock_from_world(world: &World) -> Result<Option<Self::Lock<'_>>, WorldBorrowError> {
        let ecs_type_id = match world.ecs_type_ids.get(&TypeId::of::<T>()) {
            None => return Ok(None),
            Some(ecs_type_id) => ecs_type_id,
        };

        world
            .columns
            .get(ecs_type_id)
            .map(|cell| {
                cell.try_borrow()
                    .map(|ok| cell::Ref::map(ok, |columns| columns.as_static::<T>()))
                    .map_err(|_| WorldBorrowError(type_name::<T>()))
            })
            .transpose()
    }

    fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {
        lock.0.as_slice()
    }

    fn archetype_matches(archetype: &Archetype, ecs_type_ids: &HashMap<TypeId, EcsTypeId>) -> bool {
        let ecs_type_id = match ecs_type_ids.get(&TypeId::of::<T>()) {
            Some(id) => id,
            None => return false,
        };
        archetype.column_indices.contains_key(ecs_type_id)
    }

    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        lock_borrow: &mut Self::LockBorrow<'a>,
        ecs_type_ids: &HashMap<TypeId, EcsTypeId>,
    ) -> Self::ItemIter<'a> {
        let ecs_type_id = ecs_type_ids.get(&TypeId::of::<T>()).unwrap();
        let col = archetype.column_indices[ecs_type_id];
        lock_borrow[col]
            .as_typed_storage()
            .unwrap()
            .as_vec::<T>()
            .unwrap()
            .iter()
    }

    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>> {
        iter.next()
    }

    fn get_access() -> Result<Access, ()> {
        Access::new().insert_read(TypeId::of::<T>())
    }
}

impl<T: Component> QueryParam for &'static mut T {
    type Lock<'a> = cell::RefMut<'a, StaticColumns<T>>;
    type LockBorrow<'a> = (usize, &'a mut [Vec<T>]);
    type Item<'a> = &'a mut T;
    type ItemIter<'a> = std::slice::IterMut<'a, T>;

    fn lock_from_world(world: &World) -> Result<Option<Self::Lock<'_>>, WorldBorrowError> {
        let ecs_type_id = match world.ecs_type_ids.get(&TypeId::of::<T>()) {
            Some(id) => id,
            None => return Ok(None),
        };

        world
            .columns
            .get(ecs_type_id)
            .map(|cell| {
                cell.try_borrow_mut()
                    .map(|ok| cell::RefMut::map(ok, |columns| columns.as_static_mut::<T>()))
                    .map_err(|_| WorldBorrowError(type_name::<T>()))
            })
            .transpose()
    }

    fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {
        (0, lock.0.as_mut_slice())
    }

    fn archetype_matches(archetype: &Archetype, ecs_type_ids: &HashMap<TypeId, EcsTypeId>) -> bool {
        let ecs_type_id = match ecs_type_ids.get(&TypeId::of::<T>()) {
            Some(id) => id,
            None => return false,
        };
        archetype.column_indices.contains_key(ecs_type_id)
    }

    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        (num_chopped_off, lock_borrow): &mut Self::LockBorrow<'a>,
        ecs_type_ids: &HashMap<TypeId, EcsTypeId>,
    ) -> Self::ItemIter<'a> {
        let ecs_type_id = ecs_type_ids.get(&TypeId::of::<T>()).unwrap();

        let col = archetype.column_indices[ecs_type_id];
        assert!(col >= *num_chopped_off);
        let idx = col - *num_chopped_off;
        let taken_out_borrow = std::mem::replace(lock_borrow, &mut []);
        let (chopped_of, remaining) = taken_out_borrow.split_at_mut(idx + 1);
        *lock_borrow = remaining;
        *num_chopped_off += chopped_of.len();
        chopped_of
            .last_mut()
            .unwrap()
            .as_typed_storage_mut()
            .unwrap()
            .as_vec_mut::<T>()
            .unwrap()
            .iter_mut()
    }

    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>> {
        iter.next()
    }

    fn get_access() -> Result<Access, ()> {
        Access::new().insert_write(TypeId::of::<T>())
    }
}

macro_rules! query_param_tuple_impl {
    ($($T:ident)+) => {
        impl<$($T: QueryParam),+> QueryParam for ($($T,)+) {
            type Lock<'a> = ($($T::Lock<'a>,)+);
            type LockBorrow<'a> = ($($T::LockBorrow<'a>,)+);
            type Item<'a> = ($($T::Item<'a>,)+);
            type ItemIter<'a> = ($($T::ItemIter<'a>,)+);

            fn lock_from_world(world: &World) -> Result<Option<Self::Lock<'_>>, WorldBorrowError> {
                Ok(Some(
                    ($(
                        match $T::lock_from_world(world)? {
                            None => return Ok(None),
                            Some(lock) => lock,
                        },
                    )+)
                ))
            }

            #[allow(non_snake_case)]
            fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {
                let ($($T,)+) = lock;
                ($($T::lock_borrows_from_locks($T),)+)
            }

            fn archetype_matches(archetype: &Archetype, ecs_type_ids: &HashMap<TypeId, EcsTypeId>) -> bool {
                $($T::archetype_matches(archetype, ecs_type_ids))&&+
            }

            #[allow(non_snake_case)]
            fn item_iter_from_archetype<'a>(
                archetype: &'a Archetype,
                lock_borrow: &mut Self::LockBorrow<'a>,
                ecs_type_ids: &HashMap<TypeId, EcsTypeId>,
            ) -> Self::ItemIter<'a> {
                let ($($T,)+) = lock_borrow;
                ($($T::item_iter_from_archetype(archetype, $T, ecs_type_ids),)+)
            }

            #[allow(non_snake_case)]
            fn advance_iter<'a>(iters: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>> {
                let ($($T,)+) = iters;
                Some(($($T::advance_iter($T)?,)+))
            }

            fn get_access() -> Result<Access, ()> {
                Access::from_array([$($T::get_access()),+])
            }
        }
    };
}

query_param_tuple_impl!(A B C D E F G H);
query_param_tuple_impl!(A B C D E F G);
query_param_tuple_impl!(A B C D E F);
query_param_tuple_impl!(A B C D E);
query_param_tuple_impl!(A B C D);
query_param_tuple_impl!(A B C);
query_param_tuple_impl!(A B);
query_param_tuple_impl!(A);

pub struct Maybe<Q: QueryParam>(PhantomData<Q>);
pub enum MaybeIter<'a, Q: QueryParam> {
    Some(Q::ItemIter<'a>),
    None(usize),
}
impl<Q: QueryParam> QueryParam for Maybe<Q> {
    type Lock<'a> = Option<Q::Lock<'a>>;
    type LockBorrow<'a> = Option<Q::LockBorrow<'a>>;
    type Item<'a> = Option<Q::Item<'a>>;
    type ItemIter<'a> = MaybeIter<'a, Q>;

    fn lock_from_world(world: &World) -> Result<Option<Self::Lock<'_>>, WorldBorrowError> {
        Ok(Some(Q::lock_from_world(world)?))
    }

    fn lock_borrows_from_locks<'a, 'b>(lock: &'a mut Self::Lock<'b>) -> Self::LockBorrow<'a> {
        lock.as_mut()
            .map(|q_lock| Q::lock_borrows_from_locks(q_lock))
    }

    fn archetype_matches(_: &Archetype, _: &HashMap<TypeId, EcsTypeId>) -> bool {
        true
    }

    fn item_iter_from_archetype<'a>(
        archetype: &'a Archetype,
        lock_borrow: &mut Self::LockBorrow<'a>,
        ecs_type_ids: &HashMap<TypeId, EcsTypeId>,
    ) -> Self::ItemIter<'a> {
        match Q::archetype_matches(archetype, ecs_type_ids) {
            true => MaybeIter::Some(Q::item_iter_from_archetype(
                archetype,
                lock_borrow.as_mut().unwrap(),
                ecs_type_ids,
            )),
            false => MaybeIter::None(archetype.entities.len()),
        }
    }

    fn advance_iter<'a>(iter: &mut Self::ItemIter<'a>) -> Option<Self::Item<'a>> {
        match iter {
            MaybeIter::Some(iter) => Q::advance_iter(iter).map(|item| Some(item)),
            MaybeIter::None(0) => None,
            MaybeIter::None(remaining) => {
                *remaining -= 1;
                Some(None)
            }
        }
    }

    fn get_access() -> Result<Access, ()> {
        Q::get_access()
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct DynQueryParam {
    id: EcsTypeId,
    kind: DynQueryParamKind,
}

impl DynQueryParam {
    pub fn new_ref(id: EcsTypeId) -> Self {
        Self {
            id,
            kind: DynQueryParamKind::Ref,
        }
    }

    pub fn new_mut(id: EcsTypeId) -> Self {
        Self {
            id,
            kind: DynQueryParamKind::Mut,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum DynQueryParamKind {
    Mut,
    Ref,
}

pub enum DynQueryParamLock<'a> {
    Mut(cell::RefMut<'a, DynamicColumns>),
    Ref(cell::Ref<'a, DynamicColumns>),
}

pub enum DynQueryParamLockBorrow<'a> {
    Mut(usize, &'a mut [Box<dyn ErasedBytesVec>]),
    Ref(&'a [Box<dyn ErasedBytesVec>]),
}

pub struct Query<'a, Q: QueryParam + 'static> {
    pub(crate) w: &'a World,
    pub(crate) locks: Option<(Q::Lock<'a>, Vec<DynQueryParamLock<'a>>)>,
    pub(crate) dyn_params: Vec<DynQueryParam>,
}

// TODO add `DynQueryParam::MaybeMut/Ref`
// TODO test all this stuff

impl<'b, Q: QueryParam> Query<'b, Q> {
    pub fn iter_mut(&mut self) -> QueryIter<'_, 'b, Q> {
        QueryIter::new(self)
    }

    pub fn add_dyn_param(&mut self, param: DynQueryParam) -> &mut Self {
        self.dyn_params.push(param);
        if let Some((_, dyn_locks)) = &mut self.locks {
            match param.kind {
                DynQueryParamKind::Mut => dyn_locks.push(DynQueryParamLock::Mut(
                    cell::RefMut::map(self.w.columns[&param.id].borrow_mut(), |columns| {
                        columns.as_dynamic_mut()
                    }),
                )),
                DynQueryParamKind::Ref => dyn_locks.push(DynQueryParamLock::Ref(cell::Ref::map(
                    self.w.columns[&param.id].borrow(),
                    |columns| columns.as_dynamic(),
                ))),
            }
        }

        self
    }
}
impl<'a, 'b: 'a, Q: QueryParam> IntoIterator for &'a mut Query<'b, Q> {
    type Item = Q::Item<'a>;
    type IntoIter = QueryIter<'a, 'b, Q>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

pub struct QueryIter<'a, 'b: 'a, Q: QueryParam> {
    ecs_type_ids: &'a HashMap<TypeId, EcsTypeId>,
    /// `None` if we couldnt acquire the locks because
    /// one of the columns had not been created yet
    borrows: Option<(Q::LockBorrow<'a>, Vec<DynQueryParamLockBorrow<'a>>)>,
    archetype_iter: ArchetypeIter<'a, 'b, Q>,
    item_iters: Option<(Q::ItemIter<'a>, Vec<Box<dyn Iterator<Item = *mut u8> + 'a>>)>,

    dyn_params: &'a [DynQueryParam],
    dyn_param_data_ptrs: Vec<*mut u8>,
}

type ArchetypeIter<'a, 'b: 'a, Q> = impl Iterator<Item = &'b Archetype> + 'a;
impl<'a, 'b: 'a, Q: QueryParam> QueryIter<'a, 'b, Q> {
    fn new(borrows: &'a mut Query<'b, Q>) -> Self {
        fn defining_use<'a, 'b: 'a, Q: QueryParam>(
            world: &'b World,
            dyn_params: &'a [DynQueryParam],
        ) -> ArchetypeIter<'a, 'b, Q> {
            world
                .archetypes
                .iter()
                .filter(|archetype| Q::archetype_matches(archetype, &world.ecs_type_ids))
                .filter(|archetype| {
                    dyn_params.iter().all(|param| {
                        use DynQueryParamKind::*;
                        match &param.kind {
                            Mut | Ref => archetype.column_indices.contains_key(&param.id),
                        }
                    })
                })
        }

        Self {
            ecs_type_ids: &borrows.w.ecs_type_ids,
            archetype_iter: defining_use::<Q>(borrows.w, &borrows.dyn_params[..]),
            borrows: borrows.locks.as_mut().map(|(locks, dyn_locks)| {
                (
                    Q::lock_borrows_from_locks(locks),
                    dyn_locks
                        .iter_mut()
                        .map(|lock| match lock {
                            DynQueryParamLock::Mut(r) => {
                                DynQueryParamLockBorrow::Mut(0, &mut r.0[..])
                            }
                            DynQueryParamLock::Ref(r) => DynQueryParamLockBorrow::Ref(&r.0[..]),
                        })
                        .collect::<Vec<_>>(),
                )
            }),
            item_iters: None,

            dyn_params: &borrows.dyn_params[..],
            dyn_param_data_ptrs: vec![std::ptr::null_mut(); borrows.dyn_params.len()],
        }
    }
}

impl<'a, 'b: 'a, Q: QueryParam> Iterator for QueryIter<'a, 'b, Q> {
    type Item = Q::Item<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let (borrows, dyn_borrows) = self.borrows.as_mut()?;
        'outer: loop {
            if let None = &self.item_iters {
                let archetype = self.archetype_iter.next()?;
                self.item_iters = Some((
                    Q::item_iter_from_archetype(archetype, borrows, self.ecs_type_ids),
                    self.dyn_params
                        .iter()
                        .zip(dyn_borrows.iter_mut())
                        .map(|(param, borrow)| match borrow {
                            DynQueryParamLockBorrow::Mut(num_chopped_off, storages) => {
                                let col = archetype.column_indices[&param.id];
                                assert!(col >= *num_chopped_off);
                                let idx = col - *num_chopped_off;
                                let taken_out_borrow = std::mem::replace(storages, &mut []);
                                let (chopped_of, remaining) =
                                    taken_out_borrow.split_at_mut(idx + 1);
                                *storages = remaining;
                                *num_chopped_off += chopped_of.len();
                                chopped_of
                                    .last_mut()
                                    .unwrap()
                                    .as_erased_storage_mut()
                                    .unwrap()
                                    .iter_mut()
                            }
                            DynQueryParamLockBorrow::Ref(storages) => {
                                let col = archetype.column_indices[&param.id];
                                let storage = storages[col].as_erased_storage().unwrap();
                                storage.iter()
                            }
                        })
                        .collect(),
                ));
            }

            let (static_iters, dyn_iters) = self.item_iters.as_mut().unwrap();
            match Q::advance_iter(static_iters) {
                Some(item) => {
                    for (data_ptr, iter) in self
                        .dyn_param_data_ptrs
                        .iter_mut()
                        .zip(dyn_iters.iter_mut())
                    {
                        match iter.next() {
                            None => {
                                self.item_iters = None;
                                continue 'outer;
                            }
                            Some(ptr) => {
                                *data_ptr = ptr;
                            }
                        }
                    }
                    return Some(item);
                }
                None => self.item_iters = None,
            }
        }
    }
}

impl<'a, 'b: 'a, Q: QueryParam> QueryIter<'a, 'b, Q> {
    pub fn next_dynamic(&mut self) -> Option<(<Self as Iterator>::Item, &mut [*mut u8])> {
        self.next()
            .map(|item| (item, &mut self.dyn_param_data_ptrs[..]))
    }
}

#[cfg(test)]
mod static_tests {
    use super::*;
    use crate::world::*;

    #[test]
    fn simple_query() {
        let mut world = World::new();
        let e1 = world.spawn().id();
        world.insert_component(e1, 10_u32);
        world.insert_component(e1, 12_u64);
        let e2 = world.spawn().id();
        world.insert_component(e2, 13_u64);
        world.insert_component(e2, 9_u128);

        let mut q = world.query::<&u64>().unwrap();
        let returned = q.iter_mut().collect::<Vec<_>>();
        assert_eq!(returned.as_slice(), &[&12, &13]);
    }

    #[test]
    fn tuple_query() {
        let mut world = World::new();
        let e1 = world.spawn().id();
        world.insert_component(e1, 10_u32);
        world.insert_component(e1, 12_u64);
        let e2 = world.spawn().id();
        world.insert_component(e2, 13_u64);
        world.insert_component(e2, 9_u128);

        let mut q = world.query::<(Entity, &u32, &u64)>().unwrap();
        let returned = q.iter_mut().collect::<Vec<_>>();
        assert_eq!(returned.as_slice(), &[(e1, &10, &12)]);
    }

    #[test]
    fn maybe_query() {
        let mut world = World::new();
        let e1 = world.spawn().id();
        world.insert_component(e1, 10_u32);
        world.insert_component(e1, 12_u64);
        let e2 = world.spawn().id();
        world.insert_component(e2, 13_u64);
        world.insert_component(e2, 9_u128);

        let mut q = world
            .query::<(Entity, Maybe<&u32>, &u64, Maybe<&u128>)>()
            .unwrap();
        let returned = q.iter_mut().collect::<Vec<_>>();
        assert_eq!(
            returned.as_slice(),
            &[
                (e1, Some(&10_u32), &12_u64, None),
                (e2, None, &13_u64, Some(&9_u128))
            ],
        )
    }

    #[test]
    fn query_with_despawned() {
        let mut world = World::new();
        let e1 = world.spawn().insert(10_u32).id();
        world.despawn(e1);

        let mut q = world.query::<&u32>().unwrap();
        let mut iter = q.iter_mut();
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn conflicting_queries() {
        let mut world = World::new();
        let _e1 = world.spawn().insert(10_u32).insert(10_u64).id();

        let _q1 = world.query::<&u32>().unwrap();
        assert!(matches!(world.query::<&mut u32>(), Err(_)));

        let _q2 = world.query::<&mut u64>().unwrap();
        assert!(matches!(world.query::<(&u32, &mut u64)>(), Err(_)));

        let _q3 = world.query::<&u32>().unwrap();
    }

    #[test]
    fn maybe_on_uncreated_column() {
        let mut world = World::new();
        let _e1 = world.spawn().id();
        let mut q = world.query::<Maybe<&u32>>().unwrap();
        let mut iter = q.iter_mut();
        assert_eq!(iter.next(), Some(None));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn complex_maybe_query() {
        let mut world = World::new();
        let e1 = world.spawn().insert(10_u32).id();
        let e2 = world.spawn().insert(12_u32).id();
        let mut q = world.query::<(Entity, Maybe<&u64>, &u32)>().unwrap();
        let mut iter = q.iter_mut();
        assert_eq!(iter.next(), Some((e1, None, &10_u32)));
        assert_eq!(iter.next(), Some((e2, None, &12_u32)));
        assert_eq!(iter.next(), None);
    }
}

#[cfg(test)]
mod dynamic_tests {
    use super::*;
    use crate::world::*;
    use std::alloc::Layout;

    #[test]
    fn simple_query_dynamic() {
        let mut world = World::new();
        let u32_id = world.new_dynamic_ecs_type_id(Layout::new::<u32>());
        let u64_id = world.new_dynamic_ecs_type_id(Layout::new::<u64>());
        let u128_id = world.new_dynamic_ecs_type_id(Layout::new::<u128>());

        let e1 = world.spawn().id();
        world.insert_component_dynamic(e1, u32_id, |ptr| unsafe { *(ptr.1 as *mut u32) = 10 });
        world.insert_component_dynamic(e1, u64_id, |ptr| unsafe { *(ptr.1 as *mut u64) = 12 });

        let e2 = world.spawn().id();
        world.insert_component_dynamic(e2, u64_id, |ptr| unsafe { *(ptr.1 as *mut u64) = 13 });
        world.insert_component_dynamic(e2, u128_id, |ptr| unsafe { *(ptr.1 as *mut u128) = 9 });

        let mut q = world.query::<()>().unwrap();
        q.add_dyn_param(DynQueryParam::new_ref(u64_id));

        let mut q_iter = q.iter_mut();

        let (_, r1) = q_iter.next_dynamic().unwrap();
        assert_eq!(unsafe { *(r1[0] as *mut u64) }, 12);

        let (_, r2) = q_iter.next_dynamic().unwrap();
        assert_eq!(unsafe { *(r2[0] as *mut u64) }, 13);

        assert_eq!(q_iter.next_dynamic(), None);
    }

    #[test]
    fn uncreated_column() {
        let mut world = World::new();
        let _ = world.spawn().id();
        let u32_id = world.new_dynamic_ecs_type_id(Layout::new::<u32>());

        let mut q = world.query::<()>().unwrap();
        q.add_dyn_param(DynQueryParam::new_ref(u32_id));

        let mut q_iter = q.iter_mut();
        assert_eq!(q_iter.next_dynamic(), None);
    }
}
