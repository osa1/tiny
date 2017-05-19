use std::str;

pub fn drop_port(s : &str) -> Option<&str> {
    s.find(':').map(|split| &s[ 0 .. split ])
}

#[inline]
pub fn opt_to_vec<T>(opt : Option<T>) -> Vec<T> {
    match opt {
        None => vec![],
        Some(t) => vec![t],
    }
}

////////////////////////////////////////////////////////////////////////////////

pub struct InsertIterator<'iter, A : 'iter> {
    insert_point : usize,
    current_idx  : usize,
    iter_orig    : &'iter mut Iterator<Item=A>,
    iter_insert  : &'iter mut Iterator<Item=A>,
}

impl<'iter, A> Iterator for InsertIterator<'iter, A> {
    type Item = A;

    fn next(&mut self) -> Option<A> {
        if self.current_idx >= self.insert_point {
            if let Some(a) = self.iter_insert.next() {
                Some(a)
            } else {
                self.iter_orig.next()
            }
        } else {
            self.current_idx += 1;
            self.iter_orig.next()
        }
    }
}

pub fn insert_iter<'iter, A>(iter_orig    : &'iter mut Iterator<Item=A>,
                             iter_insert  : &'iter mut Iterator<Item=A>,
                             insert_point : usize)
                             -> InsertIterator<'iter, A> {
    InsertIterator {
        insert_point: insert_point,
        current_idx: 0,
        iter_orig: iter_orig,
        iter_insert: iter_insert,
    }
}

#[cfg(test)]
mod tests {

extern crate test;

use super::*;

#[test]
fn insert_iter_test() {
    let mut range1 = 0 .. 5;
    let mut range2 = 5 .. 10;
    let iter = insert_iter(&mut range1, &mut range2, 3);
    assert_eq!(iter.collect::<Vec<i32>>(), vec![0, 1, 2, 5, 6, 7, 8, 9, 3, 4])
}

} // tests
