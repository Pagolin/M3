#[derive(Clone, Debug)]
pub enum Either<L, R> {
    Left(L),
    Right(R)
}

impl<L,R> Either<L,R> {
    pub fn left_or_panic(self) -> L {
        match self {
            Self::Left(l) => l,
            Self::Right(_) => panic!("You tried to get Either::Left from an
             Either::Right")
        }
    }
    pub fn right_or_panic(self) -> R {
        match self {
            Self::Right(r) => r,
            Self::Left(_) => panic!("You tried to get Either::Right from an\
             Either::Left")
        }
    }
    pub fn is_left(either: &Either<L, R>) -> bool {
        match either {
            Either::Left(_) => true,
            Either::Right(_) => false
        }
    }

    fn is_right(either: &Either<L, R>) -> bool {
        !Either::is_left(either)
    }
}

