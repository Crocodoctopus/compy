extern crate parking_lot;

mod bucket;
pub mod compy;
mod genvec;

use crate::compy::{Compy, CompyInsert, CompyIterate};
use std::any::TypeId;

pub trait ToOption<T, F: FnOnce() -> T> {
    fn to_option(self, f: F) -> Option<T>;
}

impl<T, F: FnOnce() -> T> ToOption<T, F> for bool {
    fn to_option(self, f: F) -> Option<T> {
        if self == true {
            Some(f())
        } else {
            None
        }
    }
}

fn main() {
    struct Pos(f32, f32);
    struct Vel(f32, f32);
    struct Acc(f32, f32);

    // init compy
    let mut compy = Compy::new(&[
        TypeId::of::<Pos>(),
        TypeId::of::<Vel>(),
        TypeId::of::<Acc>(),
    ]);

    // insert an entity
    compy.insert((Pos(0., 0.), Vel(1., 1.)));

    // iterate
    let pkey = compy.get_key(&[TypeId::of::<Pos>(), TypeId::of::<Vel>()]);
    let nkey = compy.get_key(&[TypeId::of::<Acc>()]);
    compy.iterate(pkey, nkey, |id, pos: &mut Pos, vel: &Vel| {
    	pos.0 += vel.0;
    	pos.1 += vel.1;
        pos.0 < 10.
    });

    // clean up
    compy.update();
}
