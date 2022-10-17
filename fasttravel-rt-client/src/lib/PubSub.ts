
import { EventEnvelope, Publisher } from "./Events";

export type Callback<T> = (event: T) => void;
export type UnsubscribeCallback = () => void;
export type StreamRead<T> = {
    forEach(callback: Callback<T>): Promise<void>;
    unsubscribe(): void;
}

export type Observable<T> = {
    subscribe(callback: Callback<T>): UnsubscribeCallback;
    stream(): StreamRead<T>;
}

type UnsubscribeStreamCallback<T> = (stream: StreamWrite<T>) => void;
type StreamWrite<T> = {
    write(event: T): void;
    close(): void;
}


/**
 * A minimal implementation of an Observable and a PubSub to 
 * provide basic event and stream subscriptions.
 */
export class PubSub<T> implements Publisher, Observable<T> {
    private subscriptions = new Set<Callback<T>>();
    private streams = new Set<StreamWrite<T>>();

    public observable(): Observable<T> {
        return this;
    }

    public subscribe(callback: Callback<T>): UnsubscribeCallback {
        this.subscriptions.add(callback);
        console.log("sunbscription_size: ", this.subscriptions.size);
        return (() => this.subscriptions.delete(callback));
    }

    public stream(): StreamRead<T> {
        let unsub = ((stream: StreamWrite<T>) => this.streams.delete(stream));
        let [rx, tx] = createStream(unsub);
        this.streams.add(tx);

        return rx;
    }

    public publish(env: EventEnvelope): Promise<void> {

        let msg = env.msg;
        console.log("PubSub_publish_event_msg: " + JSON.stringify(msg));

        // push to the streams
        this.streams.forEach((writer) => writer.write(msg as T));

        // execute the callbacks
        let subs = [];
        this.subscriptions.forEach((callback) => {

            let sub = new Promise<void>((resolve) => {
                callback(msg as T); // TS compile time type assertion.
                resolve();
            });

            subs.push(sub)
        });

        let ret = Promise.allSettled([subs])
            .then((results) => {
                console.info("PubSub_publish_all_settled: ", results);
                return Promise.resolve();
            });

        return ret;
    }

    public clear() {
        this.subscriptions.clear();
    }
}


// [REF]: https://developer.mozilla.org/en-US/docs/Web/API/Streams_API/Concepts
// [REF]: https://web.dev/streams/
function createStream<T>(unsubCallback: UnsubscribeStreamCallback<T>): [StreamRead<T>, StreamWrite<T>] {

    const _wStrategy = new CountQueuingStrategy({ highWaterMark: 10 });
    const _rStrategy = new CountQueuingStrategy({ highWaterMark: 100 });
    const _stream = new TransformStream<T, T>(undefined, _wStrategy, _rStrategy);
    const _writer = { write, close };
    const _reader = { forEach, unsubscribe };
    const _unsubscribeCallback = unsubCallback;


    function forEach(callback: Callback<T>): Promise<void> {

        // users must either await on each call to this function,
        // or chain multiple calls with the previous Promise.then()
        if (_stream.readable.locked) return Promise.resolve();

        const MAX_READ_PER_LOOP = 100; // [todo] add as arg to forEach
        const reader = _stream.readable.getReader();

        let reads = [];
        let n = 0;
        while (n < MAX_READ_PER_LOOP) {
            let r = reader.read()
                .then(({ done, value }) => {
                    if (done) {
                        console.log("PubSub_stream_read_done");
                        return;
                    }

                    callback(value);
                });

            reads.push(r);
            n++;
        }

        let ret = Promise.allSettled(reads).then((_results) => {
            // console.log("PubSub_stream_reads_all_settled " + JSON.stringify(_results));
        }).finally(() => reader.releaseLock());

        return ret;
    }

    function close(): void {
        _stream.readable.cancel();
        _stream.writable.close();
    }

    function unsubscribe(): void {
        close();
        _unsubscribeCallback(_writer);
    }

    function write(event: T): void {

        const writer = _stream.writable.getWriter();
        writer.ready
            .then(() => {
                writer.write(event);
            })
            .catch((err) => { console.log("PubSub_stream_write_error: " + err) })
            .finally(() => writer.releaseLock());
    }

    return [_reader, _writer]
}
