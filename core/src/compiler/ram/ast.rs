use std::collections::*;

use crate::common::expr::*;
use crate::common::input_file::InputFile;
use crate::common::input_tag::InputTag;
use crate::common::output_option::OutputOption;
use crate::common::tuple::Tuple;
use crate::common::tuple_type::TupleType;

use crate::runtime::dynamic::DynamicAggregateOp;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
  pub strata: Vec<Stratum>,
  pub relation_to_stratum: HashMap<String, usize>,
}

impl Program {
  pub fn new() -> Self {
    Self {
      strata: Vec::new(),
      relation_to_stratum: HashMap::new(),
    }
  }

  pub fn relations(&self) -> impl Iterator<Item = &Relation> {
    self.strata.iter().flat_map(|s| s.relations.values())
  }

  pub fn relation_tuple_type(&self, predicate: &str) -> Option<TupleType> {
    if let Some(stratum_id) = self.relation_to_stratum.get(predicate) {
      Some(
        self.strata[*stratum_id].relations[predicate]
          .tuple_type
          .clone(),
      )
    } else {
      None
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct Stratum {
  pub is_recursive: bool,
  pub relations: BTreeMap<String, Relation>,
  pub updates: Vec<Update>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct Relation {
  pub predicate: String,
  pub tuple_type: TupleType,
  pub input_file: Option<InputFile>,
  pub facts: Vec<Fact>,
  pub disjunctive_facts: Vec<Vec<Fact>>,
  pub output: OutputOption,
}

impl Relation {
  pub fn new(predicate: String, tuple_type: TupleType) -> Self {
    Self {
      predicate,
      tuple_type,
      input_file: None,
      facts: vec![],
      disjunctive_facts: vec![],
      output: OutputOption::Hidden,
    }
  }
}

impl std::cmp::Ord for Relation {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;

    // First compare predicate
    let pcmp = self.predicate.cmp(&other.predicate);
    if pcmp != Equal {
      return pcmp;
    };

    // Then compare tuple type
    let tcmp = self.tuple_type.cmp(&other.tuple_type);
    if tcmp != Equal {
      return tcmp;
    };

    // Then compare input file
    let icmp = self.input_file.cmp(&other.input_file);
    if icmp != Equal {
      return icmp;
    };

    // Finally compare facts
    self.facts.cmp(&other.facts)
  }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Fact {
  pub tag: InputTag,
  pub tuple: Tuple,
}

impl std::cmp::Eq for Fact {}

impl std::cmp::Ord for Fact {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    match self.partial_cmp(other) {
      Some(ord) => ord,
      _ => panic!("[Internal Error] No ordering found between facts"),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Update {
  pub target: String,
  pub dataflow: Dataflow,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dataflow {
  Unit,
  Union(Box<Dataflow>, Box<Dataflow>),
  Join(Box<Dataflow>, Box<Dataflow>),
  Intersect(Box<Dataflow>, Box<Dataflow>),
  Product(Box<Dataflow>, Box<Dataflow>),
  Antijoin(Box<Dataflow>, Box<Dataflow>),
  Difference(Box<Dataflow>, Box<Dataflow>),
  Project(Box<Dataflow>, Expr),
  Filter(Box<Dataflow>, Expr),
  Find(Box<Dataflow>, Tuple),
  Reduce(Reduce),
  Relation(String),
}

impl Dataflow {
  pub fn unit() -> Self {
    Self::Unit
  }

  pub fn union(d1: Dataflow, d2: Dataflow) -> Self {
    Self::Union(Box::new(d1), Box::new(d2))
  }

  pub fn join(d1: Dataflow, d2: Dataflow) -> Self {
    Self::Join(Box::new(d1), Box::new(d2))
  }

  pub fn intersect(d1: Dataflow, d2: Dataflow) -> Self {
    Self::Intersect(Box::new(d1), Box::new(d2))
  }

  pub fn product(d1: Dataflow, d2: Dataflow) -> Self {
    Self::Product(Box::new(d1), Box::new(d2))
  }

  pub fn antijoin(d1: Dataflow, d2: Dataflow) -> Self {
    Self::Antijoin(Box::new(d1), Box::new(d2))
  }

  pub fn difference(d1: Dataflow, d2: Dataflow) -> Self {
    Self::Difference(Box::new(d1), Box::new(d2))
  }

  pub fn project(d: Dataflow, expr: Expr) -> Self {
    Self::Project(Box::new(d), expr)
  }

  pub fn filter(d: Dataflow, expr: Expr) -> Self {
    Self::Filter(Box::new(d), expr)
  }

  pub fn find(d: Dataflow, t: Tuple) -> Self {
    Self::Find(Box::new(d), t)
  }

  pub fn reduce(op: DynamicAggregateOp, predicate: String, group_by: ReduceGroupByType) -> Self {
    Self::Reduce(Reduce {
      op,
      predicate,
      group_by,
    })
  }

  pub fn relation(r: String) -> Self {
    Self::Relation(r)
  }

  pub fn source_relations(&self) -> HashSet<&String> {
    match self {
      Self::Unit => HashSet::new(),
      Self::Union(d1, d2)
      | Self::Join(d1, d2)
      | Self::Intersect(d1, d2)
      | Self::Product(d1, d2)
      | Self::Antijoin(d1, d2)
      | Self::Difference(d1, d2) => d1
        .source_relations()
        .union(&d2.source_relations())
        .cloned()
        .collect(),
      Self::Project(d, _) | Self::Filter(d, _) | Self::Find(d, _) => d.source_relations(),
      Self::Reduce(r) => std::iter::once(r.source_relation()).collect(),
      Self::Relation(r) => std::iter::once(r).collect(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReduceGroupByType {
  None,
  Implicit,
  Join(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reduce {
  pub op: DynamicAggregateOp,
  pub predicate: String,
  pub group_by: ReduceGroupByType,
}

impl Reduce {
  pub fn source_relation(&self) -> &String {
    &self.predicate
  }
}