#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemPredicate {
    HasTypeParameters,
    TypeParameterOf,
    TypeParameterMapsTo,
    OwnsType,
    IsSendIn,
    IsSyncIn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemPredicateArgMode {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemPredicateArgKind {
    Adt,
    Type,
    Impl,
}

impl ItemPredicateArgKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Adt => "adt",
            Self::Type => "type",
            Self::Impl => "impl binding",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemPredicateArgSpec {
    pub mode: ItemPredicateArgMode,
    pub kind: ItemPredicateArgKind,
}

impl ItemPredicateArgSpec {
    const fn input(kind: ItemPredicateArgKind) -> Self {
        Self {
            mode: ItemPredicateArgMode::Input,
            kind,
        }
    }

    const fn output(kind: ItemPredicateArgKind) -> Self {
        Self {
            mode: ItemPredicateArgMode::Output,
            kind,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemPredicateSpec {
    pub predicate: Option<ItemPredicate>,
    pub name: &'static str,
    pub args: &'static [ItemPredicateArgSpec],
}

use ItemPredicateArgKind::{Adt, Impl, Type};

const NO_ARGS: &[ItemPredicateArgSpec] = &[];
const ADT_INPUT: &[ItemPredicateArgSpec] = &[ItemPredicateArgSpec::input(Adt)];
const TYPE_PARAMETER_OF_ARGS: &[ItemPredicateArgSpec] =
    &[ItemPredicateArgSpec::output(Type), ItemPredicateArgSpec::input(Adt)];
const TYPE_PARAMETER_MAPS_TO_ARGS: &[ItemPredicateArgSpec] = &[
    ItemPredicateArgSpec::input(Type),
    ItemPredicateArgSpec::output(Type),
    ItemPredicateArgSpec::input(Impl),
];
const OWNS_TYPE_ARGS: &[ItemPredicateArgSpec] = &[ItemPredicateArgSpec::input(Adt), ItemPredicateArgSpec::input(Type)];
const IS_SEND_IN_ARGS: &[ItemPredicateArgSpec] =
    &[ItemPredicateArgSpec::input(Type), ItemPredicateArgSpec::input(Impl)];
const IS_SYNC_IN_ARGS: &[ItemPredicateArgSpec] =
    &[ItemPredicateArgSpec::input(Type), ItemPredicateArgSpec::input(Impl)];

const TRUE: ItemPredicateSpec = ItemPredicateSpec {
    predicate: None,
    name: "true",
    args: NO_ARGS,
};
const FALSE: ItemPredicateSpec = ItemPredicateSpec {
    predicate: None,
    name: "false",
    args: NO_ARGS,
};
const HAS_TYPE_PARAMETERS: ItemPredicateSpec = ItemPredicateSpec {
    predicate: Some(ItemPredicate::HasTypeParameters),
    name: "has_type_parameters",
    args: ADT_INPUT,
};
const TYPE_PARAMETER_OF: ItemPredicateSpec = ItemPredicateSpec {
    predicate: Some(ItemPredicate::TypeParameterOf),
    name: "type_parameter_of",
    args: TYPE_PARAMETER_OF_ARGS,
};
const TYPE_PARAMETER_MAPS_TO: ItemPredicateSpec = ItemPredicateSpec {
    predicate: Some(ItemPredicate::TypeParameterMapsTo),
    name: "type_parameter_maps_to",
    args: TYPE_PARAMETER_MAPS_TO_ARGS,
};
const OWNS_TYPE: ItemPredicateSpec = ItemPredicateSpec {
    predicate: Some(ItemPredicate::OwnsType),
    name: "owns_type",
    args: OWNS_TYPE_ARGS,
};
const IS_SEND_IN: ItemPredicateSpec = ItemPredicateSpec {
    predicate: Some(ItemPredicate::IsSendIn),
    name: "is_send_in",
    args: IS_SEND_IN_ARGS,
};
const IS_SYNC_IN: ItemPredicateSpec = ItemPredicateSpec {
    predicate: Some(ItemPredicate::IsSyncIn),
    name: "is_sync_in",
    args: IS_SYNC_IN_ARGS,
};

pub fn item_predicate_spec(name: &str) -> Option<&'static ItemPredicateSpec> {
    match name {
        "true" => Some(&TRUE),
        "false" => Some(&FALSE),
        "has_type_parameters" => Some(&HAS_TYPE_PARAMETERS),
        "type_parameter_of" => Some(&TYPE_PARAMETER_OF),
        "type_parameter_maps_to" => Some(&TYPE_PARAMETER_MAPS_TO),
        "owns_type" => Some(&OWNS_TYPE),
        "is_send_in" => Some(&IS_SEND_IN),
        "is_sync_in" => Some(&IS_SYNC_IN),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{ItemPredicateArgKind, ItemPredicateArgMode, item_predicate_spec};

    #[test]
    fn declares_relational_output_modes() {
        let parameter_of = item_predicate_spec("type_parameter_of").expect("registered predicate");
        assert_eq!(parameter_of.args.len(), 2);
        assert_eq!(parameter_of.args[0].mode, ItemPredicateArgMode::Output);
        assert_eq!(parameter_of.args[0].kind, ItemPredicateArgKind::Type);
        assert_eq!(parameter_of.args[1].mode, ItemPredicateArgMode::Input);
        assert_eq!(parameter_of.args[1].kind, ItemPredicateArgKind::Adt);

        let maps_to = item_predicate_spec("type_parameter_maps_to").expect("registered predicate");
        assert_eq!(maps_to.args.len(), 3);
        assert_eq!(maps_to.args[1].mode, ItemPredicateArgMode::Output);
        assert_eq!(maps_to.args[2].kind, ItemPredicateArgKind::Impl);

        let is_sync_in = item_predicate_spec("is_sync_in").expect("registered predicate");
        assert_eq!(is_sync_in.args.len(), 2);
        assert_eq!(is_sync_in.args[0].mode, ItemPredicateArgMode::Input);
        assert_eq!(is_sync_in.args[0].kind, ItemPredicateArgKind::Type);
        assert_eq!(is_sync_in.args[1].kind, ItemPredicateArgKind::Impl);
    }
}
