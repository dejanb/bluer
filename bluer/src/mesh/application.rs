//! Implement Application bluetooth mesh interface

use crate::{method_call, Result, SessionInner};
use std::sync::Arc;

use dbus::{
    nonblock::{Proxy, SyncConnection},
    Path,
};
use dbus_crossroads::{Crossroads, IfaceBuilder, IfaceToken};

use crate::mesh::{Element, RegisteredElement, PATH, SERVICE_NAME, TIMEOUT};
use futures::channel::oneshot;
use std::{fmt, mem::take};

use super::{provisioner::{RegisteredProvisioner, Provisioner}, agent::ProvisionAgent};

pub(crate) const INTERFACE: &str = "org.bluez.mesh.Application1";

/// Definition of mesh application.
#[derive(Clone, Default)]
pub struct Application {
    /// Application path
    pub path: Path<'static>,
    /// Application elements
    pub elements: Vec<Element>,
    /// Provisioner
    pub provisioner: Option<Provisioner>,
}

// ---------------
// D-Bus interface
// ---------------

/// An Application exposed over D-Bus to bluez.
#[derive(Clone)]
pub struct RegisteredApplication {
    inner: Arc<SessionInner>,
    app: Application,
    agent: ProvisionAgent,
    provisioner: Option<RegisteredProvisioner>,
}

impl RegisteredApplication {
    pub(crate) fn new(inner: Arc<SessionInner>, app: Application) -> Self {
        let provisioner = match app.clone().provisioner {
            Some(prov) => Some(RegisteredProvisioner::new(inner.clone(), prov.clone())),
            None => None,
        };
        let agent = ProvisionAgent::new(inner.clone());

        Self { inner, app, provisioner, agent }
    }

    fn proxy(&self) -> Proxy<'_, &SyncConnection> {
        Proxy::new(SERVICE_NAME, PATH, TIMEOUT, &*self.inner.connection)
    }

    dbus_interface!();
    dbus_default_interface!(INTERFACE);

    pub(crate) fn register_interface(cr: &mut Crossroads) -> IfaceToken<Arc<Self>> {
        cr.register(INTERFACE, |ib: &mut IfaceBuilder<Arc<Self>>| {
            ib.method_with_cr_async("JoinComplete", ("token",), (), |ctx, cr, (_token,): (u64,)| {
                method_call(ctx, cr, move |_reg: Arc<Self>| async move {
                    println!("JoinComplete");
                    Ok(())
                })
            });
            ib.method_with_cr_async("JoinFailed", ("reason",), (), |ctx, cr, (_reason,): (String,)| {
                method_call(ctx, cr, move |_reg: Arc<Self>| async move {
                    println!("JoinFailed");
                    Ok(())
                })
            });
            cr_property!(ib, "CompanyID", _reg => {
                Some(0x05f1 as u16)
            });
            cr_property!(ib, "ProductID", _reg => {
                Some(0x0001 as u16)
            });
            cr_property!(ib, "VersionID", _reg => {
                Some(0x0001 as u16)
            });
        })
    }

    pub(crate) async fn register(
        mut self, root_path: Path<'static>, inner: Arc<SessionInner>,
    ) -> Result<ApplicationHandle> {
        {
            let mut cr = inner.crossroads.lock().await;

            let elements = take(&mut self.app.elements);

            let om = cr.object_manager();
            cr.insert(root_path.clone(), &[om], ());

            cr.insert(Path::from(format!("{}/{}", root_path.clone(), "agent")), &[inner.provision_agent_token], Arc::new(self.clone().agent));

            match self.clone().provisioner {
                Some(_) => {
                    cr.insert(self.app.path.clone(), &[inner.provisioner_token, inner.application_token], Arc::new(self.clone()))
                },
                None => {
                    cr.insert(self.app.path.clone(), &[inner.application_token], Arc::new(self.clone()))
                }
            }

            for (element_idx, element) in elements.into_iter().enumerate() {
                let element_path = element.path.clone();
                let reg_element = RegisteredElement::new(inner.clone(), element, element_idx as u8);
                //TODO register and remove all paths ... reg_paths.push(element_path.clone());
                cr.insert(element_path, &[inner.element_token], Arc::new(reg_element));
            }
        }

        let (drop_tx, drop_rx) = oneshot::channel();
        let path_unreg = root_path.clone();
        tokio::spawn(async move {
            let _ = drop_rx.await;

            log::trace!("Unpublishing application at {}", &path_unreg);
            let mut cr = inner.crossroads.lock().await;
            let _: Option<Self> = cr.remove(&path_unreg);
        });

        Ok(ApplicationHandle { name: root_path, _drop_tx: drop_tx })
    }
}

/// Handle to Application
///
/// Drop this handle to unpublish.
pub struct ApplicationHandle {
    name: dbus::Path<'static>,
    _drop_tx: oneshot::Sender<()>,
}

impl Drop for ApplicationHandle {
    fn drop(&mut self) {
        // required for drop order
    }
}

impl fmt::Debug for ApplicationHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ApplicationHandle {{ {} }}", &self.name)
    }
}
