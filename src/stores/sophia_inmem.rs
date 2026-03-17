#[derive(Clone, Copy)]
pub struct GraphIri<'a>(&'a Vec<&'a str>);

impl<'a> GraphNameMatcher for GraphIri<'a> {
    type Term = SimpleTerm<'static>;

    fn matches<T2: Term + ?Sized>(&self, graph_name: GraphName<&T2>) -> bool {
        match graph_name {
            // only include matching graph names
            Some(t) => match t.as_simple() {
                SimpleTerm::Iri(iri) => self.0.contains(&iri.as_str()),
                _ => false,
            },
            // always include the default graph
            None => true,
        }
    }
}


#[derive(Clone, Copy)]
pub struct ExclusiveGraphIri<'a>(&'a str);

impl<'a> GraphNameMatcher for ExclusiveGraphIri<'a> {
    type Term = SimpleTerm<'static>;

    fn matches<T2: Term + ?Sized>(&self, graph_name: GraphName<&T2>) -> bool {
        match graph_name {
            // only include matching graph names
            Some(t) => match t.as_simple() {
                SimpleTerm::Iri(iri) => self.0 == iri.as_str(),
                _ => false,
            },
            None => false,
        }
    }
}


#[derive(Clone, Copy)]
pub struct GraphIriName<'a>(&'a iref::Iri);

impl<'a> GraphNameMatcher for GraphIriName<'a> {
    type Term = SimpleTerm<'static>;

    fn matches<T2: Term + ?Sized>(&self, graph_name: GraphName<&T2>) -> bool {
        match graph_name {
            // only include matching graph names
            Some(t) => match t.as_simple() {
                SimpleTerm::Iri(iri) => self.0.eq(&iri.as_str()),
                _ => false,
            },
            // always include the default graph
            None => false,
        }
    }
}
