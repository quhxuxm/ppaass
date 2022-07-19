export class BackendAgentConfiguration {
    private _user_token: string | undefined;
    private _port: string | undefined;
    private _proxy_addresses: string[] | undefined;

    public get user_token(): string | undefined {
        return this._user_token;
    }
    public set user_token(value: string | undefined) {
        this._user_token = value;
    }

    public get proxy_addresses(): string[] | undefined {
        return this._proxy_addresses;
    }
    public set proxy_addresses(value: string[] | undefined) {
        this._proxy_addresses = value;
    }

    public get port(): string | undefined {
        return this._port;
    }
    public set port(value: string | undefined) {
        this._port = value;
    }
}
