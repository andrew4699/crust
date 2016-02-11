// Copyright 2015 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under (1) the MaidSafe.net Commercial License,
// version 1.0 or later, or (2) The General Public License (GPL), version 3, depending on which
// licence you accepted on initial access to the Software (the "Licences").
//
// By contributing code to the SAFE Network Software, or to this project generally, you agree to be
// bound by the terms of the MaidSafe Contributor Agreement, version 1.0.  This, along with the
// Licenses can be found in the root directory of this project at LICENSE, COPYING and CONTRIBUTOR.
//
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.
//
// Please review the Licences for the specific language governing permissions and limitations
// relating to use of the SAFE Network Software.

use utp::UtpSocket;
pub use utp_wrapper::UtpWrapper;
use std::net::UdpSocket;
use socket_addr::SocketAddr;
use std::io::Result as IoResult;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::io;
use std::time::Duration;
use maidsafe_utilities::serialisation::{deserialise, serialise};

use event::WriteEvent;

/// Connect to a peer and open a send-receive pair.  See `upgrade` for more details.
pub fn connect_utp(addr: SocketAddr) -> IoResult<(UtpWrapper, Sender<WriteEvent>)> {
    upgrade_utp(try!(UtpSocket::connect(&*addr)))
}

pub fn rendezvous_connect_utp(udp_socket: UdpSocket,
                              addr: SocketAddr)
                              -> IoResult<(UtpWrapper, Sender<WriteEvent>)> {
    upgrade_utp(try!(UtpSocket::rendezvous_connect(udp_socket, &*addr)))
}

/// Upgrades a newly connected UtpSocket to a Sender-Receiver pair that you can use to send and
/// receive objects automatically.  If there is an error decoding or encoding
/// values, that respective part is shut down.
pub fn upgrade_utp(newconnection: UtpSocket) -> IoResult<(UtpWrapper, Sender<WriteEvent>)> {
    let (output_tx, output_rx) = mpsc::channel();
    let wrapper = try!(UtpWrapper::wrap(newconnection, output_rx));

    Ok((wrapper, output_tx))
}

#[allow(unused)]
mod test {
    use super::*;
    use std::thread;
    use socket_addr::{SocketAddr, SocketAddrV4};
    use std::net::{Ipv4Addr, UdpSocket};
    use std::net;
    use std::io::Read;
    use utp::UtpListener;
    use event::WriteEvent;
    use std::io;
    use std::thread::spawn;
    use std::sync::Arc;
    use rand;

    fn listen(port: u16) -> io::Result<UtpListener> {
        UtpListener::bind(("0.0.0.0", port))
    }

    #[test]
    fn cannot_establish_connection() {
        let listener = UdpSocket::bind({
                           net::SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 0)
                       })
                           .unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let _err =
            connect_utp(SocketAddr(net::SocketAddr::V4(net::SocketAddrV4::new(Ipv4Addr::new(127,
                                                                                       0,
                                                                                       0,
                                                                                       1),
                                                                         port))))
                .err()
                .unwrap();
    }

    #[test]
    fn establishing_connection() {
        let listener = listen(0).unwrap();
        let port = listener.local_addr().unwrap().port();

        let handle = spawn(move || listener.accept().unwrap());

        // Note: when the result of connect_utp here is assigned to a variable
        // named _, this test takes much longet to complete. My guess is that
        // it happens because _ is dropped immediately, but any other named
        // variable is dropped only at the end of the scope. So when naming
        // this variable, the socket outlives the above thread, which somehow
        // makes this test finish faster for some reason.
        let _socket = connect_utp(SocketAddr(net::SocketAddr::V4(net::SocketAddrV4::new(Ipv4Addr::new(127,
                                                                                                      0,
                                                                                                      0,
                                                                                                      1),
                                                                                        port))))
                    .unwrap();

        let _ = handle.join().unwrap();
    }

    #[test]
    fn send_receive_data() {
        let listener = listen(0).unwrap();
        let port = listener.local_addr().unwrap().port();

        let th0 = spawn(move || {
            let s = listener.accept().unwrap().0;
            let (mut i, o) = upgrade_utp(s).unwrap();
            let mut buf = [0u8; 1];
            let _ = i.read(&mut buf).unwrap();
            assert_eq!(buf[0], 42);
            o.send(WriteEvent::Write(vec![43]));
        });

        let (mut i, o) =
            connect_utp(SocketAddr(net::SocketAddr::V4(net::SocketAddrV4::new(Ipv4Addr::new(127,
                                                                                       0,
                                                                                       0,
                                                                                       1),
                                                                         port))))
                .unwrap();

        let th1 = spawn(move || {
            o.send(WriteEvent::Write(vec![42]));
            let mut buf = [0u8; 1];
            let _ = i.read(&mut buf).unwrap();
            assert_eq!(buf[0], 43);
        });

        th1.join();
        th0.join();
    }

    fn loopback_v4(port: u16) -> SocketAddr {
        SocketAddr(net::SocketAddr::V4(net::SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port)))
    }

    fn duration_diff(t1: ::time::Duration, t2: ::time::Duration) -> ::time::Duration {
        if t1 >= t2 {
            t1 - t2
        } else {
            t2 - t1
        }
    }

    // On Windows, setting UdpSocket::set_read_timeout(X) causes
    // the UdpSocket::recv_from function to wait (X + E) where E
    // is ~500ms. We calculate this E here to adjust our tests.
    // See here for more info:
    // https://users.rust-lang.org/t/on-windows-udpsocket-set-read-timeout-x-waits-x-500ms/3334
    fn read_timeout_error() -> ::time::Duration {
        let mut buf = [0u8; 32];
        let s = unwrap_result!(UdpSocket::bind(&*loopback_v4(0)));

        ::time::Duration::span(|| {
            let timeout = ::std::time::Duration::from_millis(1);
            unwrap_result!(s.set_read_timeout(Some(timeout)));
            let _ = s.recv_from(&mut buf);
        })
    }
}
