use bevy::prelude::Commands;

pub trait Splittable {
    fn split(&self, quadrant: Quadrant, level: u8, commands: &mut Commands) -> Self;
}

pub struct QuadTree<T>
where
    T: Splittable,
{
    pub(crate) level: u8,
    max_depth: u8,
    parent: Option<Box<QuadTree<T>>>,
    pub(crate) children: Option<[Box<QuadTree<T>>; 4]>,
    pub(crate) node: T
}

#[derive(PartialEq, Clone)]
pub(crate) enum Quadrant {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl<T> QuadTree<T> 
where
    T: Splittable,
{
    pub fn new(depth: u8, level: u8, node: T) -> Self {
        QuadTree {
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
            Box::new(QuadTree::new(self.max_depth, child_level, self.node.split(Quadrant::TopLeft, child_level, commands))),
            Box::new(QuadTree::new(self.max_depth, child_level, self.node.split(Quadrant::TopRight, child_level, commands))),
            Box::new(QuadTree::new(self.max_depth, child_level, self.node.split(Quadrant::BottomLeft, child_level, commands))),
            Box::new(QuadTree::new(self.max_depth, child_level, self.node.split(Quadrant::BottomRight, child_level, commands))),
        ];
        self.children = Some(children);
        true
    }
    
    pub fn merge(&mut self) -> Option<[Box<QuadTree<T>>; 4]> {
        self.children.take()
    }
    
    pub fn children(&mut self) -> &mut Option<[Box<QuadTree<T>>; 4]> {
        &mut self.children
    }

    pub fn parent(&self) -> &Option<Box<QuadTree<T>>> {
        &self.parent
    }
}