extern crate parking_lot;

mod bucket;
mod byte_vec;
pub mod compy;
pub mod compy_builder;
pub mod key;

use crate::{
    compy::{CompyInsert, CompyIterate},
    compy_builder::CompyBuilder,
    key::Key,
};

fn main() {
    // :shrug:
    #[derive(Debug)]
    struct Pos(f32, f32);
    #[derive(Debug)]
    struct Vel(f32, f32);
    #[derive(Debug)]
    struct Acc(f32, f32);

    // initialize compy
    let mut compy = CompyBuilder::new()
        .with::<Pos>()
        .with::<Vel>()
        .with::<Acc>()
        .build();

    // get the keys associated with each type
    // these keys can be combined or divided with +/-
    //  ex: (pos + vel + acc - vel == pos + acc)
    let none = Key::default();
    let pos = compy.get_key_for::<Pos>();
    let vel = compy.get_key_for::<Vel>();
    let acc = compy.get_key_for::<Acc>();

    // insert an entity
    compy.insert((Pos(0., 0.), Vel(1., 1.)));
    compy.insert((Pos(0., 0.), Vel(1., 1.), Acc(2., 2.)));

    // inserts/delete all pending entities
    compy.update();

    // iterate
    // arg1: what we're searching for
    // arg2: what we're excluding
    // arg3: the closure to operate and the types.
    //  locks are automatically retrieved based on the mutability of the parameter
    compy.iterate_mut(pos + vel, acc, |pos: &mut Pos, vel: &Vel| {
        pos.0 += vel.0;
        pos.1 += vel.1;
        println!("{:?}, {:?}", pos, vel);
    });
}
