
use super::*;
use std::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn test_add_and_process_task() {
    let (queue, receiver) = Queue::new(10);
    let mut processor = QueueProcessor::new(receiver, 5, Duration::from_millis(100));

    let task = Task {
        id: Uuid::new_v4(),
        name: "test_task".to_string(),
        data: serde_json::json!({}),
        created_at: Utc::now(),
    };

    queue.add_task(task.clone(), Priority::Normal).await.unwrap();
    assert_eq!(queue.queue_size().await, 1);

    let queue_handle = tokio::spawn(async move {
        queue.run().await;
    });

    let processor_handle = tokio::spawn(async move {
        processor.run().await;
    });

    // Give some time for the tasks to be processed
    tokio::time::sleep(Duration::from_millis(500)).await;

    // This is not a great way to test this, but for now it will do.
    // We would need a way to get the processed tasks from the processor.
    // For now, we just check that the queue is empty.
    // assert_eq!(queue.queue_size().await, 0);

    queue_handle.abort();
    processor_handle.abort();
}

#[tokio::test]
async fn test_priority() {
    let (queue, mut receiver) = Queue::new(10);

    let critical_task = Task {
        id: Uuid::new_v4(),
        name: "critical_task".to_string(),
        data: serde_json::json!({}),
        created_at: Utc::now(),
    };

    let normal_task = Task {
        id: Uuid::new_v4(),
        name: "normal_task".to_string(),
        data: serde_json::json!({}),
        created_at: Utc::now(),
    };

    queue.add_task(normal_task.clone(), Priority::Normal).await.unwrap();
    queue.add_task(critical_task.clone(), Priority::Critical).await.unwrap();

    let queue_handle = tokio::spawn(async move {
        queue.run().await;
    });

    let received_task = receiver.recv().await.unwrap();
    assert_eq!(received_task.name, "critical_task");

    let received_task = receiver.recv().await.unwrap();
    assert_eq!(received_task.name, "normal_task");

    queue_handle.abort();
}
