import Dispatch
import Synchronization

/// Runs operations one at a time, in the order they were submitted. A document's edits,
/// undos, saves, and reverts must reach the engine in the order the user made them, which
/// separate tasks do not promise.
final class SerialOperations: Sendable {
    typealias Operation = @Sendable () async -> Void

    private let continuation: AsyncStream<Operation>.Continuation

    init() {
        let (stream, continuation) = AsyncStream.makeStream(of: Operation.self)
        self.continuation = continuation
        Task.detached {
            for await operation in stream { await operation() }
        }
    }

    /// Submits `operation` and returns at once.
    func submit(_ operation: @escaping Operation) {
        continuation.yield(operation)
    }

    /// Submits `operation` and waits for its result.
    func run<T: Sendable>(_ operation: @escaping @Sendable () async throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { result in
            submit {
                do {
                    result.resume(returning: try await operation())
                } catch {
                    result.resume(throwing: error)
                }
            }
        }
    }

    /// Submits `operation` and blocks the calling thread until it finishes. For NSDocument's
    /// synchronous reading, writing, and closing; `operation` must never need the caller's
    /// thread, or this deadlocks.
    func runBlocking<T: Sendable>(_ operation: @escaping @Sendable () async throws -> T) throws -> T
    {
        let done = DispatchSemaphore(value: 0)
        let outcome = Mutex<Result<T, any Error>?>(nil)
        submit {
            let result: Result<T, any Error>
            do {
                result = .success(try await operation())
            } catch {
                result = .failure(error)
            }
            outcome.withLock { $0 = result }
            done.signal()
        }
        done.wait()
        // The operation stored an outcome before signalling.
        return try outcome.withLock { $0! }.get()
    }

    /// Ends the queue once everything submitted so far has run.
    func finish() {
        continuation.finish()
    }
}
