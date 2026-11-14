//! PERMTPA ONE domain core. Pure types and rules — no I/O, no framework dependencies.

use std::{fmt, str::FromStr};

/// Organization roles. Must stay in sync with the `org_role` PostgreSQL enum
/// (enforced by `tests::matches_database_enum`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrgRole {
    Owner,
    Administrator,
    Manager,
    Accountant,
    InventoryManager,
    SalesStaff,
    ReadOnly,
}

impl OrgRole {
    pub const ALL: [OrgRole; 7] = [
        OrgRole::Owner,
        OrgRole::Administrator,
        OrgRole::Manager,
        OrgRole::Accountant,
        OrgRole::InventoryManager,
        OrgRole::SalesStaff,
        OrgRole::ReadOnly,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            OrgRole::Owner => "owner",
            OrgRole::Administrator => "administrator",
            OrgRole::Manager => "manager",
            OrgRole::Accountant => "accountant",
            OrgRole::InventoryManager => "inventory_manager",
            OrgRole::SalesStaff => "sales_staff",
            OrgRole::ReadOnly => "read_only",
        }
    }

    /// Create/edit products, variants, prices, costs and stock (mirrors `app.can_manage_inventory()`).
    pub const fn can_manage_inventory(self) -> bool {
        matches!(self, OrgRole::Owner | OrgRole::Administrator | OrgRole::Manager | OrgRole::InventoryManager)
    }

    /// See cost prices and valuation (mirrors `app.can_view_costs()`). Sales Staff and Read Only never do.
    pub const fn can_view_costs(self) -> bool {
        matches!(self, OrgRole::Owner | OrgRole::Administrator | OrgRole::Manager | OrgRole::Accountant | OrgRole::InventoryManager)
    }

    /// Whether this role may manage organization membership (mirrors `app.is_org_admin()`).
    pub const fn can_manage_members(self) -> bool {
        matches!(self, OrgRole::Owner | OrgRole::Administrator)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct UnknownRole(pub String);

impl fmt::Display for UnknownRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown role: {}", self.0)
    }
}
impl std::error::Error for UnknownRole {}

impl FromStr for OrgRole {
    type Err = UnknownRole;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        OrgRole::ALL.into_iter().find(|r| r.as_str() == s).ok_or_else(|| UnknownRole(s.to_owned()))
    }
}

/// Money is an integer count of minor units (kobo for NGN). Floats are never used for money.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Money {
    pub minor: i64,
}

impl Money {
    pub const fn from_minor(minor: i64) -> Self {
        Self { minor }
    }
    pub fn checked_add(self, rhs: Money) -> Option<Money> {
        self.minor.checked_add(rhs.minor).map(Money::from_minor)
    }
    pub fn checked_sub(self, rhs: Money) -> Option<Money> {
        self.minor.checked_sub(rhs.minor).map(Money::from_minor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_every_role() {
        for r in OrgRole::ALL {
            assert_eq!(r.as_str().parse::<OrgRole>(), Ok(r));
        }
        assert!("root".parse::<OrgRole>().is_err());
    }

    #[test]
    fn only_owner_and_admin_manage_members() {
        let allowed: Vec<_> = OrgRole::ALL.into_iter().filter(|r| r.can_manage_members()).collect();
        assert_eq!(allowed, vec![OrgRole::Owner, OrgRole::Administrator]);
    }

    #[test]
    fn inventory_permissions_match_the_policy() {
        let manage: Vec<_> = OrgRole::ALL.into_iter().filter(|r| r.can_manage_inventory()).map(|r| r.as_str()).collect();
        assert_eq!(manage, ["owner", "administrator", "manager", "inventory_manager"]);
        let costs: Vec<_> = OrgRole::ALL.into_iter().filter(|r| r.can_view_costs()).map(|r| r.as_str()).collect();
        assert_eq!(costs, ["owner", "administrator", "manager", "accountant", "inventory_manager"]);
        assert!(!OrgRole::SalesStaff.can_view_costs() && !OrgRole::ReadOnly.can_view_costs());
    }

    /// The role lists in the SQL helper functions must equal the Rust matrix (drift guard).
    #[test]
    fn inventory_permissions_match_the_sql_functions() {
        let sql = include_str!("../../../services/api/migrations/0008_inventory.sql");
        for (func, check) in [("can_manage_inventory", OrgRole::can_manage_inventory as fn(OrgRole) -> bool), ("can_view_costs", OrgRole::can_view_costs)] {
            let start = sql.find(&format!("function app.{func}()")).expect("function in migration");
            let body = &sql[start..start + sql[start..].find("$$;").expect("end")];
            let list = &body[body.find("in (").expect("role list") + 4..];
            let list = &list[..list.find(')').expect("close")];
            let mut sql_roles: Vec<&str> = list.split(',').map(|r| r.trim().trim_matches('\'')).collect();
            let mut rust_roles: Vec<&str> = OrgRole::ALL.into_iter().filter(|r| check(*r)).map(|r| r.as_str()).collect();
            sql_roles.sort_unstable();
            rust_roles.sort_unstable();
            assert_eq!(sql_roles, rust_roles, "{func} drifted between SQL and Rust");
        }
    }

    /// Drift guard: the Rust enum must list exactly the roles in the SQL migration.
    #[test]
    fn matches_database_enum() {
        let sql = include_str!("../../../services/api/migrations/0001_foundation.sql");
        let start = sql.find("create type org_role as enum (").expect("enum in migration") + "create type org_role as enum (".len();
        let end = start + sql[start..].find(')').expect("closing paren");
        let mut db: Vec<&str> = sql[start..end].split(',').map(|s| s.trim().trim_matches('\'')).collect();
        let mut rust: Vec<&str> = OrgRole::ALL.iter().map(|r| r.as_str()).collect();
        db.sort_unstable();
        rust.sort_unstable();
        assert_eq!(db, rust);
    }

    #[test]
    fn money_never_overflows_silently() {
        assert_eq!(Money::from_minor(i64::MAX).checked_add(Money::from_minor(1)), None);
        assert_eq!(Money::from_minor(10).checked_add(Money::from_minor(20)), Some(Money::from_minor(30)));
    }
}
