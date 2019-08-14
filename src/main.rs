extern crate parking_lot;

mod bucket;
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
    #[derive(Debug)]
    struct Rem(bool, u32);

    // initialize compy
    let mut compy = CompyBuilder::new()
        .with::<Pos>()
        .with::<Vel>()
        .with::<Acc>()
        .with::<Rem>()
        .build();

    // get the keys associated with each type
    // these keys can be combined or divided with +/-
    //  ex: (pos + vel + acc - vel == pos + acc)
    let none = Key::default();
    let pos = compy.get_key_for::<Pos>();
    let vel = compy.get_key_for::<Vel>();
    let acc = compy.get_key_for::<Acc>();
    let rem = compy.get_key_for::<Rem>();

    // insert an entity
    compy.insert((Rem(true, 0),));
    compy.insert((Rem(false, 1),));
    compy.insert((Rem(true, 2),));

    // inserts/delete all pending entities
    compy.update();

    // iterate
    // arg1: what we're searching for
    // arg2: what we're excluding
    // arg3: the closure to operate and the types.
    //  locks are automatically retrieved based on the mutability of the parameter
    compy.iterate_mut(rem, none, |rem: &Rem| {
        println!("{:?}", rem);
        rem.0
    });

    compy.update();

    compy.iterate_mut(rem, none, |rem: &Rem| {
        println!("O {:?}", rem);
    });
}
