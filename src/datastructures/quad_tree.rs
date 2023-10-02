use std::sync::Arc;

#[derive(Debug, Default, PartialEq)]
pub(crate) struct QuadTree<T> where T: InitializeNode {
    parent: Option<usize>,
    children: Option<[Arc<QuadTree<T>>; 4]>,
    node: T,
}

pub(crate) trait InitializeNode {
    fn new() -> Self;
}

impl<T: InitializeNode> QuadTree<T> {
    pub(crate) fn new(parent: *const QuadTree<T>, node: T) -> Self {
        QuadTree {
            parent: Some(parent as usize),
            children: None,
            node,
        }
    }

    pub(crate) fn split(&mut self) {
        if self.children.is_none() {
            self.children = Some([
                Arc::new(QuadTree::new(self as *const QuadTree<T>, T::new())),
                Arc::new(QuadTree::new(self as *const QuadTree<T>, T::new())),
                Arc::new(QuadTree::new(self as *const QuadTree<T>, T::new())),
                Arc::new(QuadTree::new(self as *const QuadTree<T>, T::new()))
            ]);
        }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_split_quadtree() {
        let mut quad_tree: QuadTree<TestNode> = QuadTree::default();
        quad_tree.split();

        assert!(quad_tree.has_children());
        assert_eq!(quad_tree.children.clone().unwrap()[0].parent().unwrap(), &quad_tree);
        assert_eq!(quad_tree.children.clone().unwrap()[1].parent().unwrap(), &quad_tree);
        assert_eq!(quad_tree.children.clone().unwrap()[2].parent().unwrap(), &quad_tree);
        assert_eq!(quad_tree.children.clone().unwrap()[3].parent().unwrap(), &quad_tree);
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

    impl InitializeNode for TestNode {
        fn new() -> Self {
            TestNode
        }
    }
}