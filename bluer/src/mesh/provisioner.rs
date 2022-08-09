//! Implement Provisioner bluetooth mesh interface

use crate::{mesh::ReqError, method_call, SessionInner};
use std::sync::Arc;

use dbus::nonblock::{Proxy, SyncConnection};
use dbus_crossroads::{Crossroads, IfaceBuilder, IfaceToken};
use tokio::sync::mpsc;

use crate::mesh::{PATH, SERVICE_NAME, TIMEOUT};

use super::application::RegisteredApplication;

pub(crate) const INTERFACE: &str = "org.bluez.mesh.Provisioner1";

/// Definition of Provisioner interface
#[derive(Clone)]
pub struct Provisioner {
    /// Control handle for provisioner once it has been registered.
    pub control_handle: ProvisionerControlHandle,
}

/// A provisioner exposed over D-Bus to bluez.
#[derive(Clone)]
pub struct RegisteredProvisioner {
    inner: Arc<SessionInner>,
    provisioner: Provisioner,
}

impl RegisteredProvisioner {
    pub(crate) fn new(inner: Arc<SessionInner>, provisioner: Provisioner) -> Self {
        Self { inner, provisioner }
    }

    fn proxy(&self) -> Proxy<'_, &SyncConnection> {
        Proxy::new(SERVICE_NAME, PATH, TIMEOUT, &*self.inner.connection)
    }

    dbus_interface!();
    dbus_default_interface!(INTERFACE);

    pub(crate) fn register_interface(cr: &mut Crossroads) -> IfaceToken<Arc<RegisteredApplication>> {
        cr.register(INTERFACE, |ib: &mut IfaceBuilder<Arc<RegisteredApplication>>| {
            ib.method_with_cr_async(
                "AddNodeComplete",
                ("uuid", "unicast", "count"),
                (),
                |ctx, cr, (_uuid, _unicast, _count): (Vec<u16>, u16, u8)| {
                    method_call(ctx, cr, move |reg: Arc<RegisteredApplication>| async move {
                        if let Some(prov) = &reg.provisioner {
                            prov.provisioner
                                .control_handle
                                .messages_tx
                                .send(ProvisionerMessage::AddNodeComplete)
                                .await
                                .map_err(|_| ReqError::Failed)?;
                        }
                        Ok(())
                    })
                },
            );
            ib.method_with_cr_async(
                "AddNodeFailed",
                ("uuid", "reason"),
                (),
                |ctx, cr, (_uuid, _reason): (Vec<u16>, String)| {
                    method_call(ctx, cr, move |reg: Arc<RegisteredApplication>| async move {
                        if let Some(prov) = &reg.provisioner {
                            prov.provisioner
                                .control_handle
                                .messages_tx
                                .send(ProvisionerMessage::AddNodeFailed)
                                .await
                                .map_err(|_| ReqError::Failed)?;
                        }
                        Ok(())
                    })
                },
            );
            ib.method_with_cr_async(
                "RequestProvData",
                ("count",),
                ("net_index", "unicast"),
                |ctx, cr, (_count,): (u8,)| {
                    method_call(ctx, cr, move |_reg: Arc<RegisteredApplication>| async move {
                        println!("RequestProvData");

                        Ok((0x000 as u16, 0x0bd as u16))
                    })
                },
            );
            cr_property!(ib, "VersionID", _reg => {
                Some(0x0001 as u16)
            });
        })
    }
}

#[derive(Clone)]
/// A handle to store inside a provisioner definition to make it controllable
/// once it has been registered.
pub struct ProvisionerControlHandle {
    /// Provisioner messages sender
    pub messages_tx: mpsc::Sender<ProvisionerMessage>,
}

#[derive(Clone, Debug)]
///Messages sent by provisioner
pub enum ProvisionerMessage {
    AddNodeComplete,
    AddNodeFailed,
}
