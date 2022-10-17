import { CoreService, CoreServiceImpl } from './CoreService'
import { ConnectionService, ConnectionServiceImpl } from './ConnectionService'
import { RealtimeModule, SessionModelRoot, RealtimeModuleConfig } from "../../pkg/fasttravel_rt_client_private"

export class SessionOptions {
  namespace: string
  workspace: string
  scopes?: Array<string>
}

export class ServiceUrls {

  // Session lambda endpoint to initiate a session with the accessToken.
  rtSessionUrl: string = "https://session.fasttravel.xyz/session/join/"

  // Check the status of collaboration space hosting.
  rtStatusUrl: string = "https://realtime.fasttravel.xyz/realtime/status/"

  // Websocket connection request endpoint.
  rtConnectUrl: string = "wss://realtime.fasttravel.xyz/realtime/connect/"
}

export class RealtimeService {

  private realtimeModule: RealtimeModule;
  private connection: ConnectionService;
  readonly core: CoreService;

  constructor(
    private accessToken: string,
    private serviceUrls?: ServiceUrls,
  ) {

    // set the default hardcoded values.
    if (this.serviceUrls === undefined) this.serviceUrls = new ServiceUrls();

    // start the WASM kernels.
    let config = RealtimeModuleConfig.new(this.accessToken, serviceUrls.rtSessionUrl, serviceUrls.rtStatusUrl, serviceUrls.rtConnectUrl);
    this.realtimeModule = RealtimeModule.new(config);
    this.connection = new ConnectionServiceImpl(this.realtimeModule);
    this.core = new CoreServiceImpl(this.realtimeModule);
  }

  public async join_session(option: SessionOptions) {

    let modelRoot = SessionModelRoot.new(option.namespace, option.workspace);
    return await this.realtimeModule.join_session(modelRoot);

  }

  public end_session() { }

}
