use std::sync::{Arc, RwLock};

#[derive(Debug, Default)]
pub(crate) struct QuadTree<T> where T: Node {
    pub(crate) parent: Option<usize>,
    pub(crate) children: Option<[Arc<RwLock<QuadTree<T>>>; 4]>,
    pub(crate) node: T,
    pub(crate) max_depth: u8,
    pub(crate) level: u8
}

pub(crate) enum Quadrant {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight
}

pub(crate) trait Node {
    fn split_node(&self, quadrant: Quadrant, level: u8) -> Self where Self: Sized;
}

pub(crate) trait Update<T> {
    fn update(&mut self, data: T);
}

pub(crate) trait NewRoot<T> {
    fn new_root(node: T) -> Self;
}

impl<T: Node> QuadTree<T> {

    pub(crate) fn new(parent: *const QuadTree<T>, node: T, max_depth: u8, level: u8) -> Self {
        QuadTree {
            parent: Some(parent as usize),
            children: None,
            node,
            max_depth,
            level
        }
    }

    pub(crate) fn split(&mut self) -> bool {
        if self.children.is_none() && self.level < self.max_depth {
            self.children = Some([
                self.new_child(Quadrant::TopLeft),
                self.new_child(Quadrant::TopRight),
                self.new_child(Quadrant::BottomLeft),
                self.new_child(Quadrant::BottomRight)
            ]);
            return true;
        }
        return false;
    }

    pub(crate) fn merge(&mut self) {
        if self.children.is_some() {
            self.children = None;
        }
    }

    pub(crate) fn parent(&self) -> Option<&QuadTree<T>> {
        unsafe {
            match self.parent {
                Some(parent) => Some((parent as *const QuadTree<T>).as_ref().expect("Parent could not be dereferenced")),
                None => None
            }
        }
    }

    pub(crate) fn has_children(&self) -> bool {
        self.children.is_some()
    }

    fn new_child(&self, quadrant: Quadrant) -> Arc<RwLock<QuadTree<T>>> {
        Arc::new(RwLock::new(QuadTree::new(self as *const QuadTree<T>, self.node.split_node(quadrant, self.level + 1), self.max_depth, self.level + 1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl PartialEq for QuadTree<TestNode> {
        fn eq(&self, other: &Self) -> bool {
            return self.node == other.node
                && self.has_children() == other.has_children()
                && self.parent() == other.parent();
        }
    }

    #[test]
    fn should_split_quadtree() {
        let mut quad_tree: QuadTree<TestNode> = QuadTree::default();
        quad_tree.split();

        assert!(quad_tree.has_children());
        assert_eq!(quad_tree.children.clone().unwrap()[0].read().unwrap().parent().unwrap(), &quad_tree);
        assert_eq!(quad_tree.children.clone().unwrap()[1].read().unwrap().parent().unwrap(), &quad_tree);
        assert_eq!(quad_tree.children.clone().unwrap()[2].read().unwrap().parent().unwrap(), &quad_tree);
        assert_eq!(quad_tree.children.clone().unwrap()[3].read().unwrap().parent().unwrap(), &quad_tree);
    }

    #[test]
    fn should_merge_quadtree() {
        let mut quad_tree: QuadTree<TestNode> = QuadTree::default();
        quad_tree.split();
        quad_tree.merge();

        assert!(!quad_tree.has_children());
    }

    #[derive(Default, PartialEq, Debug)]
    struct TestNode;

    impl Node for TestNode {
        fn split_node(&self, quadrant: Quadrant, level: u8) -> Self {
            TestNode
        }
    }
}