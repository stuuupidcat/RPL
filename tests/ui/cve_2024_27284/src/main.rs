use cassandra_cpp_sys::CassIterator as _CassIterator;
use cassandra_cpp_sys::{
    cass_false, cass_iterator_get_aggregate_meta, cass_iterator_next, cass_true,
    CassAggregateMeta as _CassAggregateMeta,
};

use std::iter::Iterator;

pub struct AggregateMeta(*const _CassAggregateMeta);

impl AggregateMeta {
    fn build(inner: *const _CassAggregateMeta) -> Self {
        if inner.is_null() {
            panic!("Unexpected null pointer")
        };
        AggregateMeta(inner)
    }
}

#[derive(Debug)]
pub struct AggregateIterator(*mut _CassIterator);

impl Iterator for AggregateIterator {
    type Item = AggregateMeta;
    #[rpl::dump_mir(dump_cfg, dump_ddg)]
    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        unsafe {
            match cass_iterator_next(self.0) {
                cass_false => None,
                cass_true => {
                    let field_value = cass_iterator_get_aggregate_meta(self.0);
                    Some(AggregateMeta::build(field_value))
                }
            }
        }
    }
}

fn main() {
    println!("Hello, world!");
}
