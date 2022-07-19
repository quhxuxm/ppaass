export default class UiAgentConfiguration {
    private _userToken: string | undefined;
    private _proxyAddresses: string[] | undefined;
    private _listeningPort: string | undefined;
    constructor() {
    }
    public get userToken(): string | undefined {
        return this._userToken;
    }
    public set userToken(value: string | undefined) {
        this._userToken = value;
    }
    public get proxyAddresses(): string[] | undefined {
        return this._proxyAddresses;
    }
    public set proxyAddresses(value: string[] | undefined) {
        this._proxyAddresses = value;
    }
    public get listeningPort(): string | undefined {
        return this._listeningPort;
    }
    public set listeningPort(value: string | undefined) {
        this._listeningPort = value;
    }
}