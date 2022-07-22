export class BackendAgentConfiguration {
    private _user_token: string | undefined;
    private _port: string | undefined;
    private _proxy_addresses: string[] | undefined;
    private _client_buffer_size: number | undefined;
    public get client_buffer_size(): number | undefined {
        return this._client_buffer_size;
    }
    public set client_buffer_size(value: number | undefined) {
        this._client_buffer_size = value;
    }
    private _message_framed_buffer_size: number | undefined;
    public get message_framed_buffer_size(): number | undefined {
        return this._message_framed_buffer_size;
    }
    public set message_framed_buffer_size(value: number | undefined) {
        this._message_framed_buffer_size = value;
    }
    private _thread_number: number | undefined;
    public get thread_number(): number | undefined {
        return this._thread_number;
    }
    public set thread_number(value: number | undefined) {
        this._thread_number = value;
    }

    private _compress: boolean | undefined;
    public get compress(): boolean | undefined {
        return this._compress;
    }
    public set compress(value: boolean | undefined) {
        this._compress = value;
    }

    private _init_proxy_connection_number: number | undefined;
    public get init_proxy_connection_number(): number | undefined {
        return this._init_proxy_connection_number;
    }
    public set init_proxy_connection_number(value: number | undefined) {
        this._init_proxy_connection_number = value;
    }
    private _min_proxy_connection_number: number | undefined;
    public get min_proxy_connection_number(): number | undefined {
        return this._min_proxy_connection_number;
    }
    public set min_proxy_connection_number(value: number | undefined) {
        this._min_proxy_connection_number = value;
    }
    private _proxy_connection_number_incremental: number | undefined;
    public get proxy_connection_number_incremental(): number | undefined {
        return this._proxy_connection_number_incremental;
    }
    public set proxy_connection_number_incremental(value: number | undefined) {
        this._proxy_connection_number_incremental = value;
    }
    private _proxy_connection_check_interval_seconds: number | undefined;
    public get proxy_connection_check_interval_seconds(): number | undefined {
        return this._proxy_connection_check_interval_seconds;
    }
    public set proxy_connection_check_interval_seconds(value: number | undefined) {
        this._proxy_connection_check_interval_seconds = value;
    }
    private _proxy_connection_check_timeout: number | undefined;
    public get proxy_connection_check_timeout(): number | undefined {
        return this._proxy_connection_check_timeout;
    }
    public set proxy_connection_check_timeout(value: number | undefined) {
        this._proxy_connection_check_timeout = value;
    }

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
