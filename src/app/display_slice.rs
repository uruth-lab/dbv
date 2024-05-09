use std::fmt::Display;

pub trait DisplaySlice {
    fn to_delimited_string(&self) -> String;
}

impl<T: Display> DisplaySlice for &[T] {
    fn to_delimited_string(&self) -> String {
        self.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl<T: Display> DisplaySlice for Vec<T> {
    fn to_delimited_string(&self) -> String {
        (&self[..]).to_delimited_string()
    }
}
