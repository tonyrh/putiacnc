//! What a click or a command on a shape really applies to: a placed text and a user group (Edit > Group) are
//! selected, moved and copied as one unit. Pure functions of the project, so the rules are tested here and the
//! window only calls them.
use crate::model::ProjectData;
use crate::shapes::GroupCopies;
use std::collections::BTreeSet;

/// The shapes of `seed` plus everything that belongs with them: the letters of a placed text and the members of a
/// user group, repeated until nothing more joins (a group can hold a text, and the text's letters can be in a group).
pub fn unit_of(d: &ProjectData, seed: &[usize]) -> Vec<usize> {
    let mut set: BTreeSet<usize> = seed.iter().copied().collect();
    loop {
        let texts: Vec<i64> = set.iter().filter_map(|&i| d.loops.get(i)?.grp).filter(|g| d.groups.iter().any(|x| x.id == *g && x.text.is_some())).collect();
        let ugs: Vec<i64> = set.iter().filter_map(|&i| d.loops.get(i)?.ug).collect();
        let before = set.len();
        for (k, l) in d.loops.iter().enumerate() {
            if l.grp.is_some_and(|g| texts.contains(&g)) || l.ug.is_some_and(|g| ugs.contains(&g)) { set.insert(k); }
        }
        if set.len() == before { break; }
    }
    set.into_iter().collect()
}

/// The selection is exactly one group already, so grouping it again would change nothing.
pub fn is_whole_group(d: &ProjectData, sel: &BTreeSet<usize>) -> bool {
    let ugs: BTreeSet<Option<i64>> = sel.iter().filter_map(|&i| d.loops.get(i)).map(|l| l.ug).collect();
    ugs.len() == 1 && ugs.iter().next().is_some_and(|g| g.is_some())
}

/// Some selected shape is in a user group.
pub fn in_group(d: &ProjectData, sel: &BTreeSet<usize>) -> bool { sel.iter().any(|&i| d.loops.get(i).is_some_and(|l| l.ug.is_some())) }

/// Grouping the selection would do something: two or more units, not already exactly one group.
pub fn can_group(d: &ProjectData, sel: &BTreeSet<usize>) -> bool {
    unit_of(d, &sel.iter().copied().collect::<Vec<_>>()).len() >= 2 && !is_whole_group(d, sel)
}

/// Edit > Group. Returns the new selection (the whole unit), or `None` when there was nothing to group.
pub fn group(d: &mut ProjectData, sel: &BTreeSet<usize>) -> Option<BTreeSet<usize>> {
    if !can_group(d, sel) { return None; }
    let all = unit_of(d, &sel.iter().copied().collect::<Vec<_>>());
    let id = GroupCopies::new(&d.loops).fresh();
    for &i in &all { if let Some(l) = d.loops.get_mut(i) { l.ug = Some(id); } }
    Some(all.into_iter().collect())
}

/// Edit > Ungroup: the groups in the selection fall apart. False when nothing was grouped.
pub fn ungroup(d: &mut ProjectData, sel: &BTreeSet<usize>) -> bool {
    if !in_group(d, sel) { return false; }
    for &i in sel { if let Some(l) = d.loops.get_mut(i) { l.ug = None; } }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shapes;

    fn three_circles() -> ProjectData {
        let mut d = shapes::blank_project().data;
        d.loops.clear();
        for x in [10.0, 30.0, 50.0] { d.loops.push(shapes::new_circle([x, 20.0], 5.0, 0)); }
        d
    }
    fn set(v: &[usize]) -> BTreeSet<usize> { v.iter().copied().collect() }

    #[test]
    fn grouping_needs_two_shapes_and_selects_them_all() {
        let mut d = three_circles();
        assert!(!can_group(&d, &set(&[0])));
        assert_eq!(group(&mut d, &set(&[0, 1])), Some(set(&[0, 1])));
        assert!(d.loops[0].ug.is_some() && d.loops[0].ug == d.loops[1].ug && d.loops[2].ug.is_none());
    }

    #[test]
    fn a_click_on_one_member_reaches_the_whole_group() {
        let mut d = three_circles();
        group(&mut d, &set(&[0, 2]));
        assert_eq!(unit_of(&d, &[2]), vec![0, 2]);
        assert_eq!(unit_of(&d, &[1]), vec![1]);
    }

    #[test]
    fn a_group_that_is_the_whole_selection_is_not_grouped_again() {
        let mut d = three_circles();
        group(&mut d, &set(&[0, 1]));
        assert!(is_whole_group(&d, &set(&[0, 1])));
        assert!(!can_group(&d, &set(&[0, 1])));
        assert_eq!(group(&mut d, &set(&[0, 1])), None);
        // adding a loose shape makes a new group around everything
        assert!(can_group(&d, &set(&[0, 1, 2])));
    }

    #[test]
    fn ungroup_frees_the_selected_members() {
        let mut d = three_circles();
        group(&mut d, &set(&[0, 1, 2]));
        assert!(ungroup(&mut d, &set(&[0, 1, 2])));
        assert!(d.loops.iter().all(|l| l.ug.is_none()));
        assert!(!ungroup(&mut d, &set(&[0, 1, 2])), "nothing left to ungroup");
    }

    #[test]
    fn each_group_gets_its_own_id() {
        let mut d = three_circles();
        group(&mut d, &set(&[0, 1]));
        let first = d.loops[0].ug;
        d.loops.push(shapes::new_circle([70.0, 20.0], 5.0, 0));
        group(&mut d, &set(&[2, 3]));
        assert_ne!(first, d.loops[2].ug);
    }
}
