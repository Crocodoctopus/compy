extern crate parking_lot;

mod bucket;
pub mod compy;
pub mod compy_builder;
pub mod key;

use crate::{
    compy::{CompyInsert, CompyIterate},
    compy_builder::CompyBuilder,
};
use std::any::TypeId;

fn main() {
    //
    #[derive(Debug)]
    struct Pos(f32, f32);
    #[derive(Debug)]
    struct Vel(f32, f32);
    #[derive(Debug)]
    struct Acc(f32, f32);

    // init Compy
    let mut compy = CompyBuilder::new()
        .with::<Pos>()
        .with::<Vel>()
        .with::<Acc>()
        .build();

    // insert an entity
    compy.insert((Pos(0., 0.), Vel(1., 1.)));

    //
    compy.update();

    // iterate
    let pkey = compy.get_key(&[TypeId::of::<Pos>(), TypeId::of::<Vel>()]);
    let nkey = compy.get_key(&[TypeId::of::<Acc>()]);
    compy.iterate_mut(pkey, nkey, |pos: &mut Pos, vel: &Vel| {
        println!("{:?}, {:?}", pos, vel);
        pos.0 += vel.0;
        pos.1 += vel.1;
    });
}
