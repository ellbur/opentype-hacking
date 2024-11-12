
use crate::glyphs::Glyph;
use keyed_priority_queue::KeyedPriorityQueue;
use float_ord::FloatOrd;
use std::collections::HashMap;
use rayon::prelude::*;
use noisy_float::prelude::*;
use crate::dictionary::Dictionary;
use std::hash::Hash;
use core::fmt::Debug;

#[derive(Clone)]
pub struct SubWithImprovement<Edit: Eq + PartialEq> {
  pub sub: Edit,
  pub improvement: u32,
  pub size_cost: f64
}

pub trait EditSystem<Edit: Eq + PartialEq, EditList> {
  fn find_improving_edits(&self, spelling: &Vec<Glyph>, pronunciation: &Vec<Glyph>, prior_rules: &EditList) -> Vec<SubWithImprovement<Edit>>;
  
  fn distance(&self, rules: &EditList, spelling: &Vec<Glyph>, pronunciation: &Vec<Glyph>) -> u32;
  fn new_distance(&self, prior_rules: &EditList, new_rule: &Edit, spelling: &Vec<Glyph>, pronunciation: &Vec<Glyph>) -> Option<u32>;
}

pub trait Table<Estimate: Clone, Estimator: Clone, Edit> {
  fn introduce(
    &self,
    frequencies_in_introducing_order: &Vec<f64>,
    introducing_i: usize,
    change_at_introducing_i: i32,
    edit: &Edit
  ) -> Estimator;
  
  fn estimate_introduce(
    &self,
    frequencies_in_introducing_order: &Vec<f64>,
    introducing_i: usize
  ) -> Estimate;
  
  fn update_edit(
    &self,
    frequencies_in_introducing_order: &Vec<f64>,
    current_distances_in_introducing_order: &Vec<u32>,
    introduced_i: usize,
    updated_i: usize,
    change_at_updated_i: i32,
    prev_estimate: Estimator
  ) -> Estimator;
}

pub trait EstimationSystem<T: Table<Estimate, Estimator, Edit>, Estimate: Clone, Estimator: Clone, Edit> {
  fn build_table(
    &self,
    frequencies_in_introducing_order: &Vec<f64>,
    current_distances_in_introducing_order: &Vec<u32>
  ) -> T;
  
  fn calc_best_possible(&self, estimate: &Estimate) -> R64;
  fn calc_worst_possible(&self, estimate: &Estimate) -> R64;
  
  fn calc_estimate(&self, estimator: &Estimator) -> Estimate;
}

#[derive(Debug)]
pub enum Outcome<Edit: Debug> {
  FoundImprovement(Edit, f64),
  FailedToFindImprovement(Edit, f64)
}

#[derive(Debug, Clone)]
pub struct WorkingEntry<Estimator> {
  estimator: Estimator,
  best_possible: f64,
  worst_possible: f64,
  size_cost: f64,
  
  // This is an index into the introducing vector,
  // which is ordered by weighted badness.
  introducing_index: usize,
  
  // This is an index into the exploring vector,
  // which is ordered by frequency.
  next_to_explore_index: usize
}

pub struct ReferenceData<'d, T, EditList> {
  dictionary: &'d Dictionary,
  rules: &'d EditList,
  
  n: usize,
  current_distances: Vec<u32>,
  
  table: T,
  
  introducing_order: Vec<usize>,
  introducing_order_rev: Vec<usize>,
  
  frequencies_in_introducing_order: Vec<f64>,
  current_distances_in_introducing_order: Vec<u32>
}

pub fn init_ref_data<
    'd,
    'h,
    T: Table<Estimate, Estimator, Edit>,
    Estimate: Clone,
    Estimator: Clone,
    EstSys: EstimationSystem<T, Estimate, Estimator, Edit>,
    EditSys: EditSystem<Edit, EditList>,
    Edit: Eq + PartialEq,
    EditList
  >(
    est_sys: &EstSys,
    edit_sys: &EditSys,
    dictionary: &'d Dictionary,
    rules: &'d EditList
  ) -> ReferenceData<'d, T, EditList>
{
  let n = dictionary.words.len();

  let current_distances: Vec<u32> = dictionary.words.iter().map(|w| {
    edit_sys.distance(rules, &w.spelling, &w.pronunciation)
  }).collect();
  
  let mut introducing_order: Vec<usize> = (0 .. n).collect();
  introducing_order.sort_by_key(|&i| FloatOrd(-dictionary.words[i].frequency * (current_distances[i] as f64)));
  let introducing_order = introducing_order;
  
  let mut introducing_order_rev = vec![0; n];
  for i in 0 .. n {
    let j = introducing_order[i];
    introducing_order_rev[j] = i;
  }

  let frequencies_in_introducing_order = (0 .. n).map(|i| dictionary.words[introducing_order[i]].frequency).collect();
  
  let current_distances_in_introducing_order = (0 .. n).map(|i| current_distances[introducing_order[i]]).collect();
  
  let table = est_sys.build_table(&frequencies_in_introducing_order, &current_distances_in_introducing_order);
  
  ReferenceData {
    dictionary,
    rules,
    
    n,
    current_distances,
    table,
    
    introducing_order,
    introducing_order_rev,
    
    frequencies_in_introducing_order,
    current_distances_in_introducing_order
  }
}

pub struct WorkingData<
    Estimator,
    Edit: Eq + PartialEq + Hash
  >
{
  working_table: HashMap<Edit, WorkingEntry<Estimator>>,
  best_possible: KeyedPriorityQueue<Edit, FloatOrd<f64>>,
  best_possible_rev: KeyedPriorityQueue<Edit, FloatOrd<f64>>,
  
  introducing_working_index: usize
}

fn improving_edits_at_i<
  'd,
  T,
  Edit: Eq + PartialEq,
  EditList,
  EditSys: EditSystem<Edit, EditList>
>(
  edit_sys: &EditSys,
  r: &ReferenceData<'d, T, EditList>,
  i: usize
) -> Vec<SubWithImprovement<Edit>>
{
  let j = r.introducing_order[i];
  let w = &r.dictionary.words[j];
  edit_sys.find_improving_edits(&w.spelling, &w.pronunciation, r.rules)
}

pub fn init_working_data<Estimator, Edit: Eq + PartialEq + Hash>() -> WorkingData<Estimator, Edit> {
  let working_table: HashMap<Edit, WorkingEntry<Estimator>> = HashMap::new();
  let best_possible: KeyedPriorityQueue<Edit, FloatOrd<f64>> = KeyedPriorityQueue::new();
    
  // This stores working items in *reverse* order by best_possible. Items that cannot be as good
  // as the worst_possible of the top of the best_possible queue should be deleted.
  let best_possible_rev: KeyedPriorityQueue<Edit, FloatOrd<f64>> = KeyedPriorityQueue::new();
  
  WorkingData {
    working_table,
    best_possible,
    best_possible_rev,
    
    introducing_working_index: 0
  }
}

fn estimate_from_introducing_iterator<
    'd,
    T: Table<Estimate, Estimator, Edit>,
    Estimate: Clone,
    Estimator: Clone,
    Edit: Eq + PartialEq + Hash,
    EditList
  >(
    r: &ReferenceData<'d, T, EditList>,
    w: &mut WorkingData<Estimator, Edit>,
    _debug: bool
  ) -> Option<Estimate>
{
  if w.introducing_working_index < r.n {
    let i = w.introducing_working_index;
    Some(r.table.estimate_introduce(&r.frequencies_in_introducing_order, i))
  }
  else {
    None
  }
}

fn introduce<
    'd,
    T: Table<Estimate, Estimator, Edit>,
    Estimate: Clone,
    Estimator: Clone,
    EstSys: EstimationSystem<T, Estimate, Estimator, Edit>,
    EditSys: EditSystem<Edit, EditList>,
    Edit: Eq + PartialEq + Hash + Clone,
    EditList
  >(
    est_sys: &EstSys,
    edit_sys: &EditSys,
    r: &ReferenceData<'d, T, EditList>,
    w: &mut WorkingData<Estimator, Edit>,
    debug: bool
  )
{
  // This occurs if either: (1) there is nothing in the heap, or (2) the
  // heap top's best_possible is not as good as best_possible_from_introducing_iterator.
  
  // Add it to the working table.
  // Note that we need to consider the introducing_index in initializing
  // best_possible and worst_possible.
  
  if w.introducing_working_index >= r.n {
    return;
  }
  
  let introducing_improving_edits = improving_edits_at_i(edit_sys, r, w.introducing_working_index);
   
  let i = w.introducing_working_index;
  
  for edit in introducing_improving_edits {
    if !w.working_table.contains_key(&edit.sub) {
      let improvement = edit.improvement;
      let change = -(improvement as i32);
      
      let init_estimator: Estimator = r.table.introduce(&r.frequencies_in_introducing_order, i, change, &edit.sub);
      let e = est_sys.calc_estimate(&init_estimator);
      let size_cost = edit.size_cost;
      let best_possible = est_sys.calc_best_possible(&e).raw() + size_cost;
      let worst_possible = est_sys.calc_worst_possible(&e).raw() + size_cost;
      
      let entry = WorkingEntry {
        estimator: init_estimator,
        best_possible,
        worst_possible,
        introducing_index: w.introducing_working_index,
        next_to_explore_index: 0,
        size_cost: edit.size_cost
      };
      
      w.working_table.insert(edit.sub.clone(), entry);
      w.best_possible.push(edit.sub.clone(), FloatOrd(-best_possible));
      w.best_possible_rev.push(edit.sub.clone(), FloatOrd(best_possible));
    }
    else {
      if debug { println!("We've seen {:?} before, skipping it.", edit.improvement); }
    }
  }
  
  w.introducing_working_index += 1;
}

fn cull_working_table<'d, 'h, Estimator, Edit: Eq + PartialEq + Hash + Debug>(w: &mut WorkingData<Estimator, Edit>, debug: bool) {
  // Use best_possible_rev to delete working table entries where the best possible is not
  // better than the worst_possible if the top of best_possible.
  if debug { println!("cull_working_table"); }
  
  let Some(best_top) = w.best_possible.peek() else { return };
  let best_worst = w.working_table.get(best_top.0).unwrap().worst_possible;
  
  if debug { println!("  best_worst = {} ({:?})", best_worst, best_top.0); }
  
  loop {
    if w.best_possible_rev.len() <= 1 {
      if debug { println!("  Only one edit, stopping cull."); }
      return;
    }
    
    let worst_best = w.best_possible_rev.peek().unwrap().1.0;
    
    if worst_best >= best_worst {
      let sub = w.best_possible_rev.peek().unwrap().0;
      if debug { println!("  Removing {:?} because {} is not better than {}", sub, worst_best, best_worst); }
      w.best_possible.remove(sub);
      w.working_table.remove(sub);
      w.best_possible_rev.pop();
    }
    else {
      if debug { println!("  Stopping cull because {} could beat {}", worst_best, best_worst); }
      return;
    }
  }
}

fn new_distance_if_new<
    EditSys: EditSystem<Edit, EditList>,
    Edit: Eq + PartialEq + Hash,
    EditList
  >(
    edit_sys: &EditSys,
    prior_edits: &EditList,
    edit: &Edit,
    spelling: &Vec<Glyph>,
    pronunciation: &Vec<Glyph>
  ) -> Option<u32>
{
  edit_sys.new_distance(prior_edits, edit, spelling, pronunciation)
}

fn change_with_new<
    EditSys: EditSystem<Edit, EditList>,
    Edit: Eq + PartialEq + Hash,
    EditList
  >(
    edit_sys: &EditSys,
    prior_edits: &EditList,
    edit: &Edit,
    spelling: &Vec<Glyph>,
    pronunciation: &Vec<Glyph>,
    orig_dist: u32
  ) -> i32
{
  match new_distance_if_new(edit_sys, prior_edits, edit, spelling, pronunciation) {
    Some(new_dist) => (new_dist as i32) - (orig_dist as i32),
    None => 0
  }
}

fn calc_change<
    EditSys: EditSystem<Edit, EditList>,
    Edit: Eq + PartialEq + Hash,
    EditList
  >(
    edit_sys: &EditSys,
    prior_edits: &EditList,
    edit: &Edit,
    spelling: &Vec<Glyph>,
    pronunciation: &Vec<Glyph>,
    orig_dist: u32
  ) -> i32
{
  change_with_new(edit_sys, prior_edits, edit, spelling, pronunciation, orig_dist)
}

fn produce_outcome<Edit: Debug + Clone>(best_possible_sub: &Edit, best_possible: f64) -> Outcome<Edit> {
  if best_possible < 0.0 {
    Outcome::FoundImprovement(best_possible_sub.clone(), best_possible)
  }
  else {
    Outcome::FailedToFindImprovement(best_possible_sub.clone(), best_possible)
  }
}

fn check_for_winner<
    'd,
    T: Table<Estimate, Estimator, Edit>,
    Estimate: Clone,
    Estimator: Clone,
    Edit: Debug + Hash + Eq + Clone,
    EditList
  >(
    r: &ReferenceData<'d, T, EditList>,
    w: &WorkingData<Estimator, Edit>,
    best_possible_from_introducing_iterator: Option<f64>,
    debug: bool
  ) -> Option<Outcome<Edit>>
{
  let best_possible_sub = w.best_possible.peek().unwrap().0.clone();
  
  if debug { println!("advancing {:?}", best_possible_sub); }
  
  let working_table_len = w.working_table.len();
  let working = w.working_table.get(&best_possible_sub).unwrap();
  
  if working_table_len == 1 && best_possible_from_introducing_iterator.map_or_else(|| true, |b| working.worst_possible <= b) {
    if debug { println!("Found the winner by priority: {:?} {}/{} {} {} {:?}", best_possible_sub, working.next_to_explore_index, r.n, working.best_possible, working.worst_possible, best_possible_from_introducing_iterator); }
    Some(produce_outcome(&best_possible_sub, working.best_possible))
  }
  else {
    let j = working.next_to_explore_index;
    
    if j >= r.n {
      // This case can really only happen due to rounding error.
      // At this point, we know:
      // * Its best_possible is equal to its worst_possible
      // * Its best_possible is at least as good as any other best_possible in the table
      // * Its best_possible is at least as good as the best possible from introducing iterator
      if debug { println!("Found the winner by default: {:?}", best_possible_sub); }
      Some(produce_outcome(&best_possible_sub, working.best_possible))
    }
    else {
      None
    }
  }
}

struct WorkIntermediate<E> {
  estimator: E,
  next_to_explore_index: usize
}

fn advance_many<
    'd,
    T: Table<Estimate, Estimator, Edit>,
    Estimate: Clone,
    Estimator: Clone,
    EditSys: EditSystem<Edit, EditList>,
    EditList,
    Edit: Eq + PartialEq + Hash
  >(
    edit_sys: &EditSys,
    r: &ReferenceData<'d, T, EditList>,
    sub: &Edit,
    working: &WorkingEntry<Estimator>,
    num_to_advance: usize,
    debug: bool
  ) -> WorkIntermediate<Estimator>
{
  let mut j = working.next_to_explore_index;
  let mut k = 0;
  let mut estimator = working.estimator.clone();
  
  while k < num_to_advance && j < r.n {
    let i = r.introducing_order_rev[j];
    
    if i == working.introducing_index {
      // Nothing to do, we've already seen this one
    }
    else {
      let word = &r.dictionary.words[j];
      
      let change = calc_change(
        edit_sys,
        r.rules,
        &sub,
        &word.spelling,
        &word.pronunciation,
        r.current_distances[j]
      );
      
      if debug {
        println!("  old distance = {}", r.current_distances[j]);
        println!("  change = {}", change);
      }
      
      estimator = r.table.update_edit(
        &r.frequencies_in_introducing_order,
        &r.current_distances_in_introducing_order,
        working.introducing_index,
        i,
        change,
        estimator
      );
    }
    
    k += 1;
    j += 1;
  }
  
  WorkIntermediate {
    estimator,
    next_to_explore_index: j
  }
}

struct AdvancingWork<Estimator, Edit> {
  edit: Edit,
  working: WorkingEntry<Estimator>
}

struct AdvancingResult<Estimator, Edit> {
  edit: Edit,
  estimator: Estimator,
  best_possible: f64,
  worst_possible: f64,
  next_to_explore_index: usize
}

fn do_work<
    'd,
     T: Table<Estimate, Estimator, Edit> + Send + Sync,
     Estimate: Clone + Send + Sync,
     Estimator: Clone + Send + Sync,
     EstSys: EstimationSystem<T, Estimate, Estimator, Edit> + Send + Sync,
     EditSys: EditSystem<Edit, EditList> + Send + Sync,
     Edit: Eq + PartialEq + Hash + Send + Sync + Debug + Clone,
     EditList: Send + Sync
   >(
     est_sys: &EstSys,
     edit_sys: &EditSys,
     r: &ReferenceData<'d, T, EditList>,
     w: &mut WorkingData<Estimator, Edit>,
     debug: bool
   )
{
  // To take advantage of multiple CPU cores, we process edits in chunks
  let edit_chunk_size = 256;
  let steps_chunk_size = 256;
  
  // Go explore the next word and update best_possible and worst_possible accordingly. Note
  // that in updating best_possible, we need to consider this entry's introducing_index,
  // since that will tell us which words it could conceivably improve.
  
  let mut edit_chunk: Vec<AdvancingWork<Estimator, Edit>> = vec![];
  for _ in 0 .. edit_chunk_size {
    if w.best_possible.is_empty() {
      break;
    }
      
    let sub = w.best_possible.pop().unwrap().0;
    let working = (*w.working_table.get(&sub).unwrap()).clone();
    edit_chunk.push(AdvancingWork {
      edit: sub,
      working
    });
  }
  
  let result_chunk: Vec<AdvancingResult<Estimator, Edit>> = edit_chunk.into_par_iter().map(|work| {
    let best_possible_sub = work.edit;
    let working = work.working;
    
    if debug { println!("advancing {:?}", best_possible_sub); }
    
    let intermediate = advance_many(edit_sys, r, &best_possible_sub, &working, steps_chunk_size, debug);
    
    let e = est_sys.calc_estimate(&intermediate.estimator);
    let size_cost = working.size_cost;
    let best_possible = est_sys.calc_best_possible(&e).raw() + size_cost;
    let worst_possible = est_sys.calc_worst_possible(&e).raw() + size_cost;
    
    AdvancingResult {
      edit: best_possible_sub,
      estimator: intermediate.estimator,
      best_possible,
      worst_possible,
      next_to_explore_index: intermediate.next_to_explore_index
    }
  }).collect();
  
  for result in result_chunk {
    w.best_possible_rev.set_priority(&result.edit, FloatOrd(result.best_possible)).unwrap();
    w.best_possible.push(result.edit.clone(), FloatOrd(-result.best_possible));
    
    let working = w.working_table.get_mut(&result.edit).unwrap();
    working.estimator = result.estimator;
    working.best_possible = result.best_possible;
    working.worst_possible = result.worst_possible;
    working.next_to_explore_index = result.next_to_explore_index;
  }
}

fn dump_state<
    'd,
    T,
    Estimator,
    EditList,
    Edit: Debug + Eq + PartialEq + Hash
  >(
    r: &ReferenceData<'d, T, EditList>,
    w: &mut WorkingData<Estimator, Edit>
  )
{
  for j in 0 .. (r.dictionary.words.len()+1) {
    for (edit, w) in w.working_table.iter() {
      if w.next_to_explore_index == j {
        println!("          {:>10} bp={:>4.1} wp={:>4.1}", format!("{:?}", edit), w.best_possible, w.worst_possible);
      }
    }
    if j < r.dictionary.words.len() {
      println!("  {:>4} f={:>4.2} d={:>3}", crate::glyphs::encode(&r.dictionary.words[j].spelling), r.dictionary.words[j].frequency, r.current_distances[j]);
    }
  }
}

pub fn step<
    'd,
    T: Table<Estimate, Estimator, Edit> + Send + Sync,
    Estimate: Clone + Send + Sync,
    Estimator: Clone + Send + Sync,
    EstSys: EstimationSystem<T, Estimate, Estimator, Edit> + Send + Sync,
    EditSys: EditSystem<Edit, EditList> + Send + Sync,
    Edit: Eq + PartialEq + Hash + Debug + Clone + Send + Sync,
    EditList: Send + Sync + Debug
  >(
    est_sys: &EstSys,
    edit_sys: &EditSys,
    r: &ReferenceData<'d, T, EditList>,
    w: &mut WorkingData<Estimator, Edit>,
    debug: bool
  ) -> Option<Outcome<Edit>>
{
  if debug { println!("step"); }
  
  if debug { dump_state(r, w); }
  
  if w.working_table.is_empty() {
    if debug { println!("Working table is empty."); }
    introduce(est_sys, edit_sys, r, w, debug);
    None
  }
  else {
    // Cull the working table by starting with the top of best_possible_rev.
    // This means that once the working table is down to a single element, and that element's
    // worst_possible is better than best_possible_from_introducing_iterator, the algorithm
    // may terminate.
    cull_working_table(w, debug);
    
    let estimate_from_introducing_iterator = estimate_from_introducing_iterator(r, w, debug);
    let best_possible_from_introducing_iterator = estimate_from_introducing_iterator.map(|e| est_sys.calc_best_possible(&e).raw());
    
    let best_possible_top = w.best_possible.peek().unwrap();
    let best_possible_from_working_table = -best_possible_top.1.0;
    
    if debug { println!("best_possible_from_working_table = {}", best_possible_from_working_table); }
    
    let intro_iter_is_better = best_possible_from_introducing_iterator.map_or_else(|| false, |b| b < best_possible_from_working_table);
    
    if debug { println!("Best possible:"); }
    if debug { println!("  from old ones: {:.2} ({:?}) {}", best_possible_from_working_table, best_possible_top.0, if intro_iter_is_better {""} else {"*"}); }
    
    if intro_iter_is_better {
      introduce(est_sys, edit_sys, r, w, debug);
      None
    }
    else {
      if let Some(res) = check_for_winner(r, w, best_possible_from_introducing_iterator, debug) {
        Some(res)
      }
      else {
        do_work(est_sys, edit_sys, r, w, debug);
        None
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashMap;
  use crate::dictionary::DictionaryWord;
  use crate::glyphs::decode;
  
  #[derive(Clone)]
  struct Test1Estimator {
    path: Vec<Test1Estimate>,
    index: usize
  }
  
  #[derive(Clone, Copy)]
  struct Test1Estimate {
    best_possible: f64,
    worst_possible: f64
  }
  
  #[derive(PartialEq, Eq, Hash, Clone, Debug)]
  struct Test1Edit {
    name: String
  }
  
  #[derive(Debug)]
  struct Test1EditList {
  }
  
  #[derive(Clone)]
  struct Test1Word {
    specific_paths: HashMap<Test1Edit, Vec<Test1Estimate>>,
    default_path: Vec<Test1Estimate>
  }
  
  struct Test1Table {
    complete_table: Vec<Test1Word>
  }
  
  impl Table<Test1Estimate, Test1Estimator, Test1Edit> for Test1Table {
    fn introduce(
      &self,
      _frequencies_in_introducing_order: &Vec<f64>,
      introducing_i: usize,
      _change_at_introducing_i: i32,
      edit: &Test1Edit
    ) -> Test1Estimator {
      let word = &self.complete_table[introducing_i];
      let path = match word.specific_paths.get(edit) {
        None => word.default_path.clone(),
        Some(path) => path.clone()
      };
      Test1Estimator {
        path,
        index: introducing_i
      }
    }
    
    fn estimate_introduce(
      &self,
      _frequencies_in_introducing_order: &Vec<f64>,
      introducing_i: usize
    ) -> Test1Estimate {
      self.complete_table[introducing_i].default_path[0]
    }
    
    fn update_edit(
      &self,
      _frequencies_in_introducing_order: &Vec<f64>,
      _current_distances_in_introducing_order: &Vec<u32>,
      _introduced_i: usize,
      updated_i: usize,
      _change_at_updated_i: i32,
      prev_estimate: Test1Estimator
    ) -> Test1Estimator {
      Test1Estimator {
        path: prev_estimate.path,
        index: updated_i
      }
    }
  }

  struct Test1EstimationSystem {
    complete_table: Vec<Test1Word>
  }
  
  impl EstimationSystem<Test1Table, Test1Estimate, Test1Estimator, Test1Edit> for Test1EstimationSystem {
    fn build_table(
      &self,
      _frequencies_in_introducing_order: &Vec<f64>,
      _current_distances_in_introducing_order: &Vec<u32>
    ) -> Test1Table {
      Test1Table {
        complete_table: self.complete_table.clone()
      }
    }
    
    fn calc_estimate(&self, estimator: &Test1Estimator) -> Test1Estimate {
      estimator.path[estimator.index]
    }
    
    fn calc_best_possible(&self, e: &Test1Estimate) -> R64 { r64(e.best_possible) }
    fn calc_worst_possible(&self, e: &Test1Estimate) -> R64 { r64(e.worst_possible) }
  }
  
  struct Test1EditSystem {
    improving_edits: HashMap<(Vec<Glyph>, Vec<Glyph>), Vec<SubWithImprovement<Test1Edit>>>,
    old_distances: HashMap<(Vec<Glyph>, Vec<Glyph>), u32>,
    new_distances: HashMap<(Vec<Glyph>, Vec<Glyph>, Test1Edit), Option<u32>>,
  }
  
  impl EditSystem<Test1Edit, Test1EditList> for Test1EditSystem {
    fn find_improving_edits(
        &self,
        spelling: &Vec<Glyph>,
        pronunciation: &Vec<Glyph>,
        _prior_rules: &Test1EditList
      ) -> Vec<SubWithImprovement<Test1Edit>>
    {
      self.improving_edits.get(
        &(spelling.clone(), pronunciation.clone())
      )
        .unwrap()
        .clone()
    }
    
    fn distance(&self, _rules: &Test1EditList, spelling: &Vec<Glyph>, pronunciation: &Vec<Glyph>) -> u32 {
      self.old_distances.get(&(spelling.clone(), pronunciation.clone())).unwrap().clone()
    }
    
    fn new_distance(&self, _prior_rules: &Test1EditList, new_rule: &Test1Edit, spelling: &Vec<Glyph>, pronunciation: &Vec<Glyph>) -> Option<u32> {
      self.new_distances.get(&(spelling.clone(), pronunciation.clone(), new_rule.clone())).unwrap().clone()
    }
  }
  
  #[test]
  fn genastarlike_test_1() {
    let edit_sys = Test1EditSystem {
      improving_edits: vec![
        (
          (decode("to"), decode("tu")),
          vec![
            SubWithImprovement {
              sub: Test1Edit { name: "foo".to_owned() },
              improvement: 1,
              size_cost: 0.0
            },
            SubWithImprovement {
              sub: Test1Edit { name: "bar".to_owned() },
              improvement: 1,
              size_cost: 0.0
            },
          ]
        )
      ].into_iter().collect(),
      
      old_distances: vec![
        (
          (decode("to"), decode("tu")),
          1
        ),
        (
          (decode("who"), decode("hu")),
          1
        ),
      ].into_iter().collect(),
      
      new_distances: vec![
        (
          ( decode("to"), decode("tu"), Test1Edit { name: "foo".to_owned() }),
          Some(0)
        ),
        (
          ( decode("to"), decode("tu"), Test1Edit { name: "bar".to_owned() }),
          Some(0)
        ),
        (
          ( decode("who"), decode("hu"), Test1Edit { name: "foo".to_owned() }),
          Some(0)
        ),
        (
          ( decode("who"), decode("hu"), Test1Edit { name: "bar".to_owned() }),
          Some(1)
        ),
      ].into_iter().collect()
    };
    
    let est_sys = Test1EstimationSystem {
      complete_table: vec![
        Test1Word {
          specific_paths: [
            (
              Test1Edit { name: "foo".to_owned() },
              vec![
                Test1Estimate { best_possible: -1.0, worst_possible:  1.0 },
                Test1Estimate { best_possible: -1.0, worst_possible: -1.0 }
              ],
            ),
            (
              Test1Edit { name: "bar".to_owned() },
              vec![
                Test1Estimate { best_possible: -1.0, worst_possible:  1.0 },
                Test1Estimate { best_possible: -0.5, worst_possible: -0.5 }
              ],
            ),
          ].iter().cloned().collect(),
          default_path: vec![
            Test1Estimate { best_possible: 5.0, worst_possible: 6.0 },
            Test1Estimate { best_possible: 7.0, worst_possible: 8.0 }
          ]
        },
        Test1Word {
          specific_paths: [].iter().cloned().collect(),
          default_path: vec![
            Test1Estimate { best_possible: 5.0, worst_possible: 6.0 },
            Test1Estimate { best_possible: 7.0, worst_possible: 8.0 }
          ]
        },
      ]
    };
    
    let dictionary = Dictionary {
      words: vec![
        DictionaryWord {
          spelling: decode("to"),
          pronunciation: decode("tu"),
          frequency: 1.0
        },
        DictionaryWord {
          spelling: decode("who"),
          pronunciation: decode("hu"),
          frequency: 1.0
        },
      ]
    };
    
    println!("Initializing ref data...");
    let rules = Test1EditList { };
    
    // est_sys: &EstSys,
    // edit_sys: &EditSys,
    // dictionary: &'d Dictionary,
    // rules: &'d EditList,
    // mid: u32
    let r = init_ref_data(&est_sys, &edit_sys, &dictionary, &rules);
    
    println!("Initializing working data...");
    let mut w = init_working_data();
    
    println!("current_distances = {:?}", r.current_distances);
    println!("introducing_order = {:?}", r.introducing_order);
    println!("");
    
    let mut the_winner: Option<Test1Edit> = None;
    
    for _ in 0 .. 10 {
      if let Some(outcome) = step(&est_sys, &edit_sys, &r, &mut w, true) {
        if let Outcome::FoundImprovement(winner, _) = outcome {
          the_winner = Some(winner);
          break;
        }
        else {
          panic!("Failed to find improvement");
        }
      }
      println!("");
    }
    
    assert_eq!(the_winner.unwrap(), Test1Edit { name: "foo".to_owned() });
  }
}

