use bevy::math::Vec3;
use bevy::prelude::Commands;

pub struct Octree<T: Splittable> {
    pub(crate) level: u8,
    max_depth: u8,
    parent: Option<Box<Octree<T>>>,
    pub(crate) children: Option<[Box<Octree<T>>; 8]>,
    pub(crate) node: T
}

pub trait Splittable {
    fn split(&self, octant: Octant, level: u8, commands: &mut Commands) -> Self;
}

pub enum Octant {
    TopLeftFront,
    TopLeftBack,
    TopRightFront,
    TopRightBack,
    BottomLeftFront,
    BottomLeftBack,
    BottomRightFront,
    BottomRightBack
}

impl<T> Octree<T>
where
    T: Splittable,
{
    pub fn new(depth: u8, level: u8, node: T) -> Self {
        Octree {
            level,
            max_depth: depth,
            parent: None,
            children: None,
            node
        }
    }

    pub fn split(&mut self, commands: &mut Commands) -> bool {
        if self.level == self.max_depth {
            return false;
        }

        let child_level = self.level + 1;
        let children = [
            Box::new(Octree::new(self.max_depth, child_level, self.node.split(Octant::TopLeftFront, child_level, commands))),
            Box::new(Octree::new(self.max_depth, child_level, self.node.split(Octant::TopLeftBack, child_level, commands))),
            Box::new(Octree::new(self.max_depth, child_level, self.node.split(Octant::TopRightFront, child_level, commands))),
            Box::new(Octree::new(self.max_depth, child_level, self.node.split(Octant::TopRightBack, child_level, commands))),
            Box::new(Octree::new(self.max_depth, child_level, self.node.split(Octant::BottomLeftFront, child_level, commands))),
            Box::new(Octree::new(self.max_depth, child_level, self.node.split(Octant::BottomLeftBack, child_level, commands))),
            Box::new(Octree::new(self.max_depth, child_level, self.node.split(Octant::BottomRightFront, child_level, commands))),
            Box::new(Octree::new(self.max_depth, child_level, self.node.split(Octant::BottomRightBack, child_level, commands))),
        ];
        self.children = Some(children);
        true
    }

    pub fn merge(&mut self) -> Option<[Box<Octree<T>>; 8]> {
        self.children.take()
    }

    pub fn children(&mut self) -> &mut Option<[Box<Octree<T>>; 8]> {
        &mut self.children
    }

    pub fn parent(&self) -> &Option<Box<Octree<T>>> {
        &self.parent
    }
}

impl Octant {
    /// Given a face, returns the center position of the octant relative to a
    /// unit cube centered at the origin. Octants always point in the same direction,
    /// regardless of the face. For example, the `TopLeftFront` octant will always
    /// point towards the top left front relative to bevy's coordinate system.
    pub fn center_on_unit_cube(&self) -> Vec3 {
        match self {
            Octant::TopLeftFront => Vec3::new(-0.5, 0.5, 0.5),
            Octant::TopLeftBack => Vec3::new(-0.5, 0.5, -0.5),
            Octant::TopRightFront => Vec3::new(0.5, 0.5, 0.5),
            Octant::TopRightBack => Vec3::new(0.5, 0.5, -0.5),
            Octant::BottomLeftFront => Vec3::new(-0.5, -0.5, 0.5),
            Octant::BottomLeftBack => Vec3::new(-0.5, -0.5, -0.5),
            Octant::BottomRightFront => Vec3::new(0.5, -0.5, 0.5),
            Octant::BottomRightBack => Vec3::new(0.5, -0.5, -0.5),
        }
    }
}