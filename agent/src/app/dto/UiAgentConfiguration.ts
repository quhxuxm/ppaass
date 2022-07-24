export default class UiAgentConfiguration {
    private _userToken: string | undefined;
    private _proxyAddresses: string[] | undefined;
    private _listeningPort: string | undefined;
    private _clientBufferSize: number | undefined;
    private _enableCompressing: boolean | undefined;
    public get enableCompressing(): boolean | undefined {
        return this._enableCompressing;
    }
    public set enableCompressing(value: boolean | undefined) {
        this._enableCompressing = value;
    }
    private _agentThreadNumber: number | undefined;
    public get agentThreadNumber(): number | undefined {
        return this._agentThreadNumber;
    }
    public set agentThreadNumber(value: number | undefined) {
        this._agentThreadNumber = value;
    }
    public get clientBufferSize(): number | undefined {
        return this._clientBufferSize;
    }
    public set clientBufferSize(value: number | undefined) {
        this._clientBufferSize = value;
    }
    private _messageFramedBufferSize: number | undefined;
    public get messageFramedBufferSize(): number | undefined {
        return this._messageFramedBufferSize;
    }
    public set messageFramedBufferSize(value: number | undefined) {
        this._messageFramedBufferSize = value;
    }
    private _initProxyConnectionNumber: number | undefined;
    public get initProxyConnectionNumber(): number | undefined {
        return this._initProxyConnectionNumber;
    }
    public set initProxyConnectionNumber(value: number | undefined) {
        this._initProxyConnectionNumber = value;
    }
    private _minProxyConnectionNumber: number | undefined;
    public get minProxyConnectionNumber(): number | undefined {
        return this._minProxyConnectionNumber;
    }
    public set minProxyConnectionNumber(value: number | undefined) {
        this._minProxyConnectionNumber = value;
    }
    private _proxyConnectionNumberIncremental: number | undefined;
    public get proxyConnectionNumberIncremental(): number | undefined {
        return this._proxyConnectionNumberIncremental;
    }
    public set proxyConnectionNumberIncremental(value: number | undefined) {
        this._proxyConnectionNumberIncremental = value;
    }
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

    private _proxyConnectionCheckInterval: number | undefined;
    public get proxyConnectionCheckInterval(): number | undefined {
        return this._proxyConnectionCheckInterval;
    }
    public set proxyConnectionCheckInterval(value: number | undefined) {
        this._proxyConnectionCheckInterval = value;
    }
    private _proxyConnectionCheckTimeout: number | undefined;
    public get proxyConnectionCheckTimeout(): number | undefined {
        return this._proxyConnectionCheckTimeout;
    }
    public set proxyConnectionCheckTimeout(value: number | undefined) {
        this._proxyConnectionCheckTimeout = value;
    }
}
