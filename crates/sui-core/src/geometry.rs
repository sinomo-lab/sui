use std::ops::{Add, AddAssign, Sub, SubAssign};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub const fn to_vector(self) -> Vector {
        Vector::new(self.x, self.y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vector {
    pub x: f32,
    pub y: f32,
}

impl Vector {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl Add<Vector> for Point {
    type Output = Point;

    fn add(self, rhs: Vector) -> Self::Output {
        Point::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign<Vector> for Point {
    fn add_assign(&mut self, rhs: Vector) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub<Point> for Point {
    type Output = Vector;

    fn sub(self, rhs: Point) -> Self::Output {
        Vector::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Add for Vector {
    type Output = Vector;

    fn add(self, rhs: Vector) -> Self::Output {
        Vector::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign for Vector {
    fn add_assign(&mut self, rhs: Vector) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for Vector {
    type Output = Vector;

    fn sub(self, rhs: Vector) -> Self::Output {
        Vector::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl SubAssign for Vector {
    fn sub_assign(&mut self, rhs: Vector) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self::new(0.0, 0.0);

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    pub fn clamp(self, min: Size, max: Size) -> Self {
        Self::new(
            self.width.clamp(min.width, max.width),
            self.height.clamp(min.height, max.height),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    pub const fn from_origin_size(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    pub fn from_points(min: Point, max: Point) -> Self {
        Self::new(
            min.x,
            min.y,
            (max.x - min.x).max(0.0),
            (max.y - min.y).max(0.0),
        )
    }

    pub const fn x(self) -> f32 {
        self.origin.x
    }

    pub const fn y(self) -> f32 {
        self.origin.y
    }

    pub const fn width(self) -> f32 {
        self.size.width
    }

    pub const fn height(self) -> f32 {
        self.size.height
    }

    pub fn max_x(self) -> f32 {
        self.origin.x + self.size.width
    }

    pub fn max_y(self) -> f32 {
        self.origin.y + self.size.height
    }

    pub fn is_empty(self) -> bool {
        self.size.is_empty()
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x()
            && point.x <= self.max_x()
            && point.y >= self.y()
            && point.y <= self.max_y()
    }

    pub fn translate(self, delta: Vector) -> Self {
        Self::from_origin_size(self.origin + delta, self.size)
    }

    pub fn inflate(self, x: f32, y: f32) -> Self {
        Self::new(
            self.origin.x - x,
            self.origin.y - y,
            self.size.width + (x * 2.0),
            self.size.height + (y * 2.0),
        )
    }

    pub fn intersection(self, other: Rect) -> Option<Self> {
        let min_x = self.x().max(other.x());
        let min_y = self.y().max(other.y());
        let max_x = self.max_x().min(other.max_x());
        let max_y = self.max_y().min(other.max_y());

        if min_x >= max_x || min_y >= max_y {
            return None;
        }

        Some(Self::from_points(
            Point::new(min_x, min_y),
            Point::new(max_x, max_y),
        ))
    }

    pub fn union(self, other: Rect) -> Self {
        if self.is_empty() {
            return other;
        }

        if other.is_empty() {
            return self;
        }

        Self::from_points(
            Point::new(self.x().min(other.x()), self.y().min(other.y())),
            Point::new(
                self.max_x().max(other.max_x()),
                self.max_y().max(other.max_y()),
            ),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub xx: f32,
    pub yx: f32,
    pub xy: f32,
    pub yy: f32,
    pub dx: f32,
    pub dy: f32,
}

impl Transform {
    pub const IDENTITY: Self = Self::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);

    pub const fn new(xx: f32, yx: f32, xy: f32, yy: f32, dx: f32, dy: f32) -> Self {
        Self {
            xx,
            yx,
            xy,
            yy,
            dx,
            dy,
        }
    }

    pub const fn translation(x: f32, y: f32) -> Self {
        Self::new(1.0, 0.0, 0.0, 1.0, x, y)
    }

    pub const fn translation_vector(delta: Vector) -> Self {
        Self::translation(delta.x, delta.y)
    }

    pub const fn scale(x: f32, y: f32) -> Self {
        Self::new(x, 0.0, 0.0, y, 0.0, 0.0)
    }

    pub fn rotation(radians: f32) -> Self {
        let (sin, cos) = radians.sin_cos();
        Self::new(cos, sin, -sin, cos, 0.0, 0.0)
    }

    pub const fn is_identity(self) -> bool {
        self.xx == 1.0
            && self.yx == 0.0
            && self.xy == 0.0
            && self.yy == 1.0
            && self.dx == 0.0
            && self.dy == 0.0
    }

    pub fn then(self, next: Self) -> Self {
        Self::new(
            (next.xx * self.xx) + (next.xy * self.yx),
            (next.yx * self.xx) + (next.yy * self.yx),
            (next.xx * self.xy) + (next.xy * self.yy),
            (next.yx * self.xy) + (next.yy * self.yy),
            (next.xx * self.dx) + (next.xy * self.dy) + next.dx,
            (next.yx * self.dx) + (next.yy * self.dy) + next.dy,
        )
    }

    pub fn transform_point(self, point: Point) -> Point {
        Point::new(
            (self.xx * point.x) + (self.xy * point.y) + self.dx,
            (self.yx * point.x) + (self.yy * point.y) + self.dy,
        )
    }

    pub fn transform_vector(self, vector: Vector) -> Vector {
        Vector::new(
            (self.xx * vector.x) + (self.xy * vector.y),
            (self.yx * vector.x) + (self.yy * vector.y),
        )
    }

    pub fn transform_rect_bbox(self, rect: Rect) -> Rect {
        if rect.is_empty() {
            return rect;
        }

        let top_left = self.transform_point(rect.origin);
        let top_right = self.transform_point(Point::new(rect.max_x(), rect.y()));
        let bottom_left = self.transform_point(Point::new(rect.x(), rect.max_y()));
        let bottom_right = self.transform_point(Point::new(rect.max_x(), rect.max_y()));

        let min_x = top_left
            .x
            .min(top_right.x)
            .min(bottom_left.x)
            .min(bottom_right.x);
        let min_y = top_left
            .y
            .min(top_right.y)
            .min(bottom_left.y)
            .min(bottom_right.y);
        let max_x = top_left
            .x
            .max(top_right.x)
            .max(bottom_left.x)
            .max(bottom_right.x);
        let max_y = top_left
            .y
            .max(top_right.y)
            .max(bottom_left.y)
            .max(bottom_right.y);

        Rect::from_points(Point::new(min_x, min_y), Point::new(max_x, max_y))
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::{Point, Rect, Transform, Vector};

    #[test]
    fn point_and_vector_math_is_stable() {
        let origin = Point::new(10.0, 20.0);
        let delta = Vector::new(5.0, -2.0);

        assert_eq!(origin + delta, Point::new(15.0, 18.0));
        assert_eq!((origin + delta) - origin, delta);
    }

    #[test]
    fn rect_intersection_returns_overlap() {
        let left = Rect::new(0.0, 0.0, 10.0, 10.0);
        let right = Rect::new(8.0, 2.0, 10.0, 10.0);

        assert_eq!(
            left.intersection(right),
            Some(Rect::new(8.0, 2.0, 2.0, 8.0))
        );
    }

    #[test]
    fn transform_composition_applies_in_order() {
        let transform = Transform::scale(2.0, 3.0).then(Transform::translation(5.0, 7.0));

        assert_eq!(
            transform.transform_point(Point::new(4.0, 2.0)),
            Point::new(13.0, 13.0)
        );
    }

    #[test]
    fn transform_rect_bbox_covers_rotated_rect() {
        let rect = Rect::new(0.0, 0.0, 10.0, 4.0);
        let bbox = Transform::rotation(std::f32::consts::FRAC_PI_2).transform_rect_bbox(rect);

        assert!((bbox.x() + 4.0).abs() < 0.001);
        assert!(bbox.y().abs() < 0.001);
        assert!((bbox.width() - 4.0).abs() < 0.001);
        assert!((bbox.height() - 10.0).abs() < 0.001);
    }
}
