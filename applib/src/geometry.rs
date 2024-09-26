use num::{Signed, Num, Integer, Float};
use num::cast::AsPrimitive;
use core::ops;
use core::convert::{TryFrom, TryInto};

pub trait Coord: 'static + Signed + Copy + Num + PartialOrd {}

impl Coord for i64 {}
impl Coord for f32 {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2D<T: Coord> {
    pub x: T,
    pub y: T,
}

#[derive(Debug, Clone, Copy)]
pub struct Vec2D<T: Coord> {
    pub x: T,
    pub y: T,
}

impl<T: Coord> ops::Sub<Point2D<T>> for Point2D<T> {
    type Output = Vec2D<T>;
    fn sub(self, pt: Point2D<T>) -> Self::Output {
        Self::Output {
            x: self.x - pt.x,
            y: self.y - pt.y,
        }
    }
}

impl<T: Coord> ops::Add<Vec2D<T>> for Point2D<T> {
    type Output = Point2D<T>;
    fn add(self, vec: Vec2D<T>) -> Self::Output {
        Self::Output {
            x: self.x + vec.x,
            y: self.y + vec.y,
        }
    }
}

impl<T: Coord> ops::Sub<Vec2D<T>> for Point2D<T> {
    type Output = Point2D<T>;
    fn sub(self, vec: Vec2D<T>) -> Self::Output {
        Self::Output {
            x: self.x - vec.x,
            y: self.y - vec.y,
        }
    }
}

impl<T: Coord> ops::Mul<T> for Vec2D<T> {
    type Output = Vec2D<T>;
    fn mul(self, f: T) -> Self::Output {
        Self::Output {
            x: f * self.x,
            y: f * self.y,
        }
    }
}

impl<T: Coord> ops::Sub<Vec2D<T>> for Vec2D<T> {
    type Output = Vec2D<T>;
    fn sub(self, vec: Vec2D<T>) -> Self::Output {
        Self::Output {
            x: self.x - vec.x,
            y: self.y - vec.y,
        }
    }
}

impl<T: Coord> ops::Add<Vec2D<T>> for Vec2D<T> {
    type Output = Vec2D<T>;
    fn add(self, vec: Vec2D<T>) -> Self::Output {
        Self::Output {
            x: self.x + vec.x,
            y: self.y + vec.y,
        }
    }
}

impl<T: Coord> Vec2D<T> {

    pub fn zero() -> Self {
        Self { x: T::zero(), y: T::zero() }
    }

    pub fn cross(&self, vec: Vec2D<T>) -> T {
        self.x * vec.y - vec.x * self.y
    }
}


impl Vec2D<f32> {
    pub fn round_to_int(&self) -> Vec2D<i64> {
        Vec2D::<i64> {
            x: f32::round(self.x) as i64,
            y: f32::round(self.y) as i64,
        }
    }
}

impl Point2D<f32> {
    pub fn round_to_int(&self) -> Point2D<i64> {
        Point2D::<i64> {
            x: f32::round(self.x) as i64,
            y: f32::round(self.y) as i64,
        }
    }
}


impl Vec2D<i64> {
    pub fn to_float(&self) -> Vec2D<f32> {
        Vec2D::<f32> {
            x: self.x as f32,
            y: self.y as f32,
        }
    }
}

impl Point2D<i64> {
    pub fn to_float(&self) -> Point2D<f32> {
        Point2D::<f32> {
            x: self.x as f32,
            y: self.y as f32,
        }
    }
}

pub struct Triangle2D<T: Coord> {
    pub points: [Point2D<T>; 3]
}

impl<T: Coord> Triangle2D<T> {

    pub fn check_is_inside(&self, p: Point2D<T>) -> bool {

        let [p0, p1, p2] = self.points;

        let cp0 = (p - p0).cross(p1 - p0);
        let cp1 = (p - p1).cross(p2 - p1);
        let cp2 = (p - p2).cross(p0 - p2);
        
        let zero = T::zero();

        (cp0 < zero) && (cp1 < zero) && (cp2 < zero)
    }
}


pub struct Quad2D<T: Coord> {
    pub points: [Point2D<T>; 4]
}

impl<T: Coord> Quad2D<T> {

    pub fn triangles(&self) -> (Triangle2D<T>, Triangle2D<T>) {

        let [p0, p1, p2, p3] = self.points;

        let tri0 = Triangle2D::<T> { points: [p0, p1, p3] };
        let tri1 = Triangle2D::<T> { points: [p1, p2, p3] };

        (tri0, tri1)
    }


    pub fn check_is_inside(&self, p: Point2D<T>) -> bool {
        let (tri0, tri1) = self.triangles();
        tri0.check_is_inside(p) || tri1.check_is_inside(p)
    }
}
