//! Implement Provisioner bluetooth provisoner agent

use crate::{mesh::ReqError, method_call, SessionInner};
use std::sync::Arc;

use dbus::nonblock::{Proxy, SyncConnection};
use dbus_crossroads::{Crossroads, IfaceBuilder, IfaceToken};
use hex::FromHex;

use crate::mesh::{PATH, SERVICE_NAME, TIMEOUT};
use std::io::stdin;

pub(crate) const INTERFACE: &str = "org.bluez.mesh.ProvisionAgent1";

#[derive(Clone)]
/// Implements org.bluez.mesh.ProvisionAgent1 interface
pub struct ProvisionAgent {
    inner: Arc<SessionInner>,
}

impl ProvisionAgent {
    pub(crate) fn new(inner: Arc<SessionInner>) -> Self {
        Self { inner }
    }

    fn proxy(&self) -> Proxy<'_, &SyncConnection> {
        Proxy::new(SERVICE_NAME, PATH, TIMEOUT, &*self.inner.connection)
    }

    dbus_interface!();
    dbus_default_interface!(INTERFACE);

    pub(crate) fn register_interface(cr: &mut Crossroads) -> IfaceToken<Arc<Self>> {
        cr.register(INTERFACE, |ib: &mut IfaceBuilder<Arc<Self>>| {
            ib.method_with_cr_async(
                "DisplayNumeric",
                ("type", "value"),
                (),
                |ctx, cr, (_type, value): (String, u32)| {
                    method_call(ctx, cr, move |_reg: Arc<Self>| async move {
                        println!("Enter '{:?}' on the remote device!", value);
                        Ok(())
                    })
                },
            );
            ib.method_with_cr_async("PromptStatic", ("type",), ("value",), |ctx, cr, (_type,): (String,)| {
                method_call(ctx, cr, move |_reg: Arc<Self>| async move {
                    println!("Please input the value displayed on the device that is beaing provisioned: ");
                    let mut input_string = String::new();
                    stdin().read_line(&mut input_string).ok().expect("Failed to read input!");
                    let hex = Vec::from_hex(input_string.trim()).map_err(|_| ReqError::Failed)?;
                    Ok((hex,))
                })
            });
            cr_property!(ib, "Capabilities", _reg => {
                // TODO configure capabilities
                let mt: Vec<String> = vec!["out-numeric".into(), "static-oob".into()];
                Some(mt)
            });
        })
    }
}
