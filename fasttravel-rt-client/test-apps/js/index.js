
import { createClient, ServiceUrls, SessionOptions, FasttravelClient, RealtimeServices, CoreService, CoreEventMessage } from "../../src/index.ts"

// client info
const clientID = 'client_id_01';
const clientAPIKEY = 'CLIENT_PUBLIC_API_KEY_PLACEHOLDER';

// user info
const userID = 'user_id_01';
const userSECRET = 'user_secret_01';

async function authUser() {
    const auth_url = 'http://localhost:27002/authorize/';
    const authResponse = await fetch(
        auth_url, {
        method: 'POST',
        headers: {
            'Accept': 'application/json',
            'Content-Type': 'application/json'
        },
        body: JSON.stringify({
            user_id: userID,
            user_secret: userSECRET,
            client_id: clientID,
            audience: 'realtime:api',
        },)
    }
    );

    const response = await authResponse.json();
    console.log(response);

    return response.access_token
}

async function connectRealtime() {
    // authorize user
    const accessToken = await authUser();

    // create realtime-client
    let serviceUrls = new ServiceUrls();
    serviceUrls.rtSessionUrl = "http://localhost:27001/session/join/";
    serviceUrls.rtStatusUrl = "http://localhost:27000/realtime/status/";
    serviceUrls.rtConnectUrl = "ws://localhost:27000/realtime/connect/";
    const client = createClient(clientAPIKEY, accessToken, serviceUrls);

    // connect realtime
    let opt = new SessionOptions();
    opt.namespace = "org@name";
    opt.workspace = "root@document";
    await client.realtime.join_session(opt);

    // subscribe to events
    const unsub1 = client.realtime.core.events().subscribe((msg) => {
        console.log("Subscription_1_message: " + JSON.stringify(msg));
    });
    const unsub2 = client.realtime.core.events().subscribe((msg) => {
        console.log("Subscription_2_message: " + JSON.stringify(msg));
    });

    // subscribe to streams
    const stream = client.realtime.core.events().stream();

    return [client, stream]
}

function sendTextMessage(client, msg) {
    // send text message to server
    client.realtime.core.sendTextMessage(msg);
}

async function runTest() {
    const testString = "_loop-message_";
    const bigString = testString + "_x_".repeat(10);
    console.log("___runTest___");

    const [client, stream] = await connectRealtime();

    console.time("___loop_message_timer___");
    sendTextMessage(client, bigString + 0);

    // /*
    // [...Array(10)].forEach((_, i) => {
    //     sendTextMessage(client, bigString + i);
    // });
    // */

    console.timeEnd("___loop_message_timer___");

    await stream.forEach((msg) => {
        console.log("Stream_message: " + JSON.stringify(msg));
    });

    await stream.forEach((msg) => {
        console.log("Stream_message: " + JSON.stringify(msg));
    });
}

async function run() {
    await runTest();
    console.log("exiting Test");
}

run()
