extern crate futures;
extern crate tokio_mock_task;
extern crate tokio_sync;

use tokio_sync::mpsc;
use tokio_mock_task::*;

use futures::prelude::*;

use std::thread;

trait AssertSend: Send {}
impl AssertSend for mpsc::Sender<i32> {}
impl AssertSend for mpsc::Receiver<i32> {}

macro_rules! assert_ready {
    ($e:expr) => {{
        match $e {
            Ok(futures::Async::Ready(v)) => v,
            Ok(_) => panic!("not ready"),
            Err(e) => panic!("error = {:?}", e),
        }
    }}
}

macro_rules! assert_not_ready {
    ($e:expr) => {{
        match $e {
            Ok(futures::Async::NotReady) => {},
            Ok(futures::Async::Ready(v)) => panic!("ready; value = {:?}", v),
            Err(e) => panic!("error = {:?}", e),
        }
    }}
}

#[test]
fn send_recv_with_buffer() {
    let (tx, rx) = mpsc::channel::<i32>(16);
    let mut rx = rx.wait();

    tx.send(1).wait().unwrap();

    assert_eq!(rx.next().unwrap(), Ok(1));
}

#[test]
fn send_recv_buffer_limited() {
    let (mut tx, mut rx) = mpsc::channel::<i32>(1);
    let mut task = MockTask::new();

    // Run on a task context
    task.enter(|| {
        assert!(tx.poll_complete().unwrap().is_ready());
        assert!(tx.poll_ready().unwrap().is_ready());

        // Send first message
        let res = tx.start_send(1).unwrap();
        assert!(is_ready(&res));
        assert!(tx.poll_ready().unwrap().is_not_ready());

        // Send second message
        let res = tx.start_send(2).unwrap();
        assert!(!is_ready(&res));

        // Take the value
        assert_eq!(rx.poll().unwrap(), Async::Ready(Some(1)));
        assert!(tx.poll_ready().unwrap().is_ready());

        let res = tx.start_send(2).unwrap();
        assert!(is_ready(&res));
        assert!(tx.poll_ready().unwrap().is_not_ready());

        // Take the value
        assert_eq!(rx.poll().unwrap(), Async::Ready(Some(2)));
        assert!(tx.poll_ready().unwrap().is_ready());
    });
}

#[test]
fn send_shared_recv() {
    let (tx1, rx) = mpsc::channel::<i32>(16);
    let tx2 = tx1.clone();
    let mut rx = rx.wait();

    tx1.send(1).wait().unwrap();
    assert_eq!(rx.next().unwrap(), Ok(1));

    tx2.send(2).wait().unwrap();
    assert_eq!(rx.next().unwrap(), Ok(2));
}

#[test]
fn send_recv_threads() {
    let (tx, rx) = mpsc::channel::<i32>(16);
    let mut rx = rx.wait();

    thread::spawn(move|| {
        tx.send(1).wait().unwrap();
    });

    assert_eq!(rx.next().unwrap(), Ok(1));
}

#[test]
fn recv_close_gets_none_idle() {
    let (mut tx, mut rx) = mpsc::channel::<i32>(10);
    let mut task = MockTask::new();

    rx.close();

    task.enter(|| {
        assert_eq!(rx.poll(), Ok(Async::Ready(None)));
        assert!(tx.poll_ready().is_err());
    });
}

#[test]
fn recv_close_gets_none_reserved() {
    let (mut tx1, mut rx) = mpsc::channel::<i32>(1);
    let mut tx2 = tx1.clone();

    assert_ready!(tx1.poll_ready());

    let mut task = MockTask::new();

    task.enter(|| {
        assert_not_ready!(tx2.poll_ready());
    });

    rx.close();

    assert!(task.is_notified());

    task.enter(|| {
        assert!(tx2.poll_ready().is_err());
        assert_not_ready!(rx.poll());
    });

    assert!(!task.is_notified());

    assert!(tx1.try_send(123).is_ok());

    assert!(task.is_notified());

    task.enter(|| {
        let v = assert_ready!(rx.poll());
        assert_eq!(v, Some(123));

        let v = assert_ready!(rx.poll());
        assert!(v.is_none());
    });
}

#[test]
fn tx_close_gets_none() {
    let (_, mut rx) = mpsc::channel::<i32>(10);
    let mut task = MockTask::new();

    // Run on a task context
    task.enter(|| {
        let v = assert_ready!(rx.poll());
        assert!(v.is_none());
    });
}

fn is_ready<T>(res: &AsyncSink<T>) -> bool {
    match *res {
        AsyncSink::Ready => true,
        _ => false,
    }
}

#[test]
fn try_send_fail() {
    let (mut tx, rx) = mpsc::channel(1);
    let mut rx = rx.wait();

    tx.try_send("hello").unwrap();

    // This should fail
    assert!(tx.try_send("fail").is_err());

    assert_eq!(rx.next(), Some(Ok("hello")));

    tx.try_send("goodbye").unwrap();
    drop(tx);

    assert_eq!(rx.next(), Some(Ok("goodbye")));
    assert!(rx.next().is_none());
}

#[test]
#[ignore]
fn drop_tx_with_permit_releases_permit() {
    // poll_ready reserves capacity, ensure that the capacity is released if tx
    // is dropped w/o sending a value.
}
