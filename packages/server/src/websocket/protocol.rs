use yrs::sync::{Awareness, Error, Message, Protocol};
use yrs::{Transact, Update};

use crate::documents::permissions::Permission;

pub struct PermissionedProtocol {
    pub permission: Permission,
}

impl Protocol for PermissionedProtocol {
    fn handle_update(
        &self,
        awareness: &mut Awareness,
        update: Update,
    ) -> Result<Option<Message>, Error> {
        if self.permission < Permission::Edit {
            tracing::warn!("Rejecting update from non-edit user");
            return Err(Error::PermissionDenied {
                reason: "Insufficient permissions to edit".into(),
            });
        }
        // Delegate to default behavior
        let mut txn = awareness.doc().transact_mut();
        txn.apply_update(update);
        Ok(None)
    }
}
