#![feature(generic_associated_types)]
//! Attach and send/receive BT Mesh messages
//!
//! Example meshd
//! [burrboard/gateway]$ sudo /usr/libexec/bluetooth/bluetooth-meshd --config ${PWD}/deploy/bluez/example/meshcfg --storage ${PWD}/deploy/bluez/example/mesh --debug
//!
//! Example send
//! [bluer/bluer-tools]$ RUST_LOG=TRACE cargo run --example mesh_sensor_server -- --token dae519a06e504bd3
//!
//! Example receive
//! [bluer/bluer-tools]$ RUST_LOG=TRACE cargo run --example mesh_sensor_client -- --token 7eb48c91911361da

use bluer::mesh::{
    application::{Application, ApplicationMessage},
    element::*,
    node::Node,
};
use btmesh_common::{opcode::Opcode, CompanyIdentifier, ParseError};
use btmesh_models::{
    sensor::{PropertyId, SensorConfig, SensorData, SensorDescriptor, SensorMessage, SensorServer, SensorStatus},
    Message, Model,
};
use clap::Parser;
use dbus::Path;
use futures::StreamExt;
use std::sync::Arc;
use tokio::{
    signal,
    sync::mpsc,
    time::{self, sleep, Duration},
};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    token: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let args = Args::parse();
    let session = bluer::Session::new().await?;

    let mesh = session.mesh().await?;

    let (_element_control, element_handle) = element_control();
    let (app_tx, app_rx) = mpsc::channel(1);

    let root_path = Path::from("/mesh_server");
    let app_path = Path::from(format!("{}/{}", root_path.clone(), "application"));
    let element_path = Path::from(format!("{}/{}", root_path.clone(), "ele00"));

    let sim = Application {
        path: app_path,
        elements: vec![Element {
            path: element_path.clone(),
            location: None,
            models: vec![Arc::new(FromDrogue::new(BoardSensor::new()))],
            control_handle: Some(element_handle),
        }],
        events_tx: app_tx,
        provisioner: None,
    };

    let registered = mesh.application(root_path.clone(), sim.clone()).await?;

    let mut node: Option<Node> = None;

    match args.token {
        Some(token) => {
            println!("Attaching with token {}", token);
            node = Some(mesh.attach(root_path.clone(), &token).await?);
        }
        None => {
            let device_id = Uuid::new_v4();
            println!("Joining device: {}", device_id.as_simple());

            mesh.join(root_path.clone(), device_id).await?;
        }
    }

    println!("{:?}", sim.path);

    println!("Sensor server ready. Press enter to send a message. Press Ctrl+C to quit");

    let (messages_tx, mut messages_rx) = mpsc::channel(10);
    let lines_messages_tx = messages_tx.clone();
    let mut app_stream = ReceiverStream::new(app_rx);

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(16));

        loop {
            interval.tick().await;
            _ = messages_tx.send(generate_message()).await;
        }
    });

    std::thread::spawn(move || loop {
        let mut line = String::new();
        std::io::stdin().read_line(&mut line).unwrap();
        _ = lines_messages_tx.blocking_send(generate_message());
    });

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                break
            },
            Some(message) = messages_rx.recv() => {
                if let Some(ref n) = node {
                    n.publish::<BoardSensor>(message, element_path.clone()).await?;
                }
            },
            app_evt = app_stream.next() => {
                match app_evt {
                    Some(msg) => {
                        match msg {
                            ApplicationMessage::JoinComplete(token) => {
                                println!("Joined with token {:016x}", token);
                                println!("Attaching");
                                node = Some(mesh.attach(root_path.clone(), &format!("{:016x}", token)).await?);
                            },
                            ApplicationMessage::JoinFailed(reason) => {
                                println!("Failed to join: {}", reason);
                                break;
                            },
                        }
                    },
                    None => break,
                }
            }
        }
    }

    println!("Shutting down");
    drop(registered);
    sleep(Duration::from_secs(1)).await;

    Ok(())
}

fn generate_message() -> BoardSensorMessage {
    SensorMessage::Status(SensorStatus::new(Temperature(21.0)))
}

#[derive(Clone, Debug)]
pub struct SensorModel;

#[derive(Clone, Debug, Default)]
pub struct Temperature(f32);

impl SensorConfig for SensorModel {
    type Data = Temperature;

    const DESCRIPTORS: &'static [SensorDescriptor] = &[SensorDescriptor::new(PropertyId(0x4F), 1)];
}

impl SensorData for Temperature {
    fn decode(&mut self, id: PropertyId, params: &[u8]) -> Result<(), ParseError> {
        if id.0 == 0x4F {
            self.0 = params[0] as f32 / 2.0;
            Ok(())
        } else {
            Err(ParseError::InvalidValue)
        }
    }

    fn encode<const N: usize>(
        &self, _: PropertyId, xmit: &mut heapless::Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        let value = (self.0 * 2 as f32) as u8;
        xmit.extend_from_slice(&value.to_le_bytes()).map_err(|_| InsufficientBuffer)?;
        Ok(())
    }
}

type BoardSensor = SensorServer<SensorModel, 1, 1>;
type BoardSensorMessage = SensorMessage<SensorModel, 1, 1>;

const COMPANY_IDENTIFIER: CompanyIdentifier = CompanyIdentifier(0x05F1);
const COMPANY_MODEL: ModelIdentifier = ModelIdentifier::Vendor(COMPANY_IDENTIFIER, 0x0001);

#[derive(Clone, Debug)]
pub struct VendorModel;

impl Model for VendorModel {
    const IDENTIFIER: ModelIdentifier = COMPANY_MODEL;
    type Message = VendorMessage;

    fn parse(_opcode: Opcode, _parameters: &[u8]) -> Result<Option<Self::Message>, ParseError> {
        unimplemented!();
    }
}

#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum VendorMessage {}

impl Message for VendorMessage {
    fn opcode(&self) -> Opcode {
        unimplemented!();
    }

    fn emit_parameters<const N: usize>(
        &self, _xmit: &mut heapless::Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        unimplemented!();
    }
}
