import { BackendAgentServerStatus } from './../dto/BackendServerStatus';
import { BackendAgentConfiguration } from '../dto/BackendAgentConfiguration';
import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api';
import { Event } from '@tauri-apps/api/event';
import { appWindow } from '@tauri-apps/api/window';
import { from, map, Observable } from 'rxjs';
import UiAgentConfiguration from '../dto/UiAgentConfiguration';

const RETRIVE_AGENT_CONFIGURATION_BACKEND_COMMAND = "retrive_agent_configuration";
const RETRIVE_AGENT_SERVER_STATUS_BACKEND_COMMAND = "retrive_agent_server_status";
const SAVE_AGENT_CONFIGURATION_BACKEND_COMMAND = "save_agent_server_config";
const START_AGENT_SERVER_BACKEND_COMMAND = "start_agent_server";
const STOP_AGENT_SERVER_BACKEND_COMMAND = "stop_agent_server";
const AGENT_SERVER_STOP_BACKEND_EVENT = "agent-server-stop-backend-event";
const AGENT_SERVER_START_BACKEND_EVENT = "agent-server-start-backend-event";

@Injectable({
    providedIn: 'root'
})
export class BackendService {
    private agentStarted: boolean;

    constructor() {
        this.agentStarted = false;
    }

    public loadAgentConfiguration(): Observable<UiAgentConfiguration> {
        return from(invoke<BackendAgentConfiguration>(RETRIVE_AGENT_CONFIGURATION_BACKEND_COMMAND)).pipe(map(item => {
            let uiConfiguration = new UiAgentConfiguration();
            uiConfiguration.listeningPort = item.port;
            uiConfiguration.proxyAddresses = item.proxy_addresses;
            uiConfiguration.userToken = item.user_token;
            uiConfiguration.agentThreadNumber = item.thread_number;
            uiConfiguration.clientBufferSize = item.client_buffer_size;
            uiConfiguration.messageFramedBufferSize = item.message_framed_buffer_size;
            uiConfiguration.enableCompressing = item.compress;
            uiConfiguration.initProxyConnectionNumber = item.init_proxy_connection_number;
            uiConfiguration.minProxyConnectionNumber = item.min_proxy_connection_number;
            uiConfiguration.proxyConnectionNumberIncremental = item.proxy_connection_number_incremental;
            uiConfiguration.proxyConnectionCheckInterval = item.proxy_connection_check_interval_seconds;
            uiConfiguration.proxyConnectionCheckTimeout = item.proxy_connection_check_timeout
            return uiConfiguration;
        }));
    }

    public listenToAgentServerStart(callback: (event: Event<any>) => void): void {
        appWindow.listen(AGENT_SERVER_START_BACKEND_EVENT, (event) => {
            callback(event)
        })
    }

    public listenToAgentServerStop(callback: (event: Event<any>) => void): void {
        appWindow.listen(AGENT_SERVER_STOP_BACKEND_EVENT, (event) => {
            callback(event)
        })
    }

    public saveAgentConfiguration(uiConfiguration: UiAgentConfiguration): Observable<any> {
        let backendConfiguration = new BackendAgentConfiguration();
        backendConfiguration.port = uiConfiguration.listeningPort?.toString();
        backendConfiguration.proxy_addresses = uiConfiguration.proxyAddresses;
        backendConfiguration.user_token = uiConfiguration.userToken;
        backendConfiguration.compress = uiConfiguration.enableCompressing;
        backendConfiguration.client_buffer_size = uiConfiguration.clientBufferSize;
        backendConfiguration.message_framed_buffer_size = uiConfiguration.messageFramedBufferSize;
        backendConfiguration.init_proxy_connection_number = uiConfiguration.initProxyConnectionNumber;
        backendConfiguration.min_proxy_connection_number = uiConfiguration.minProxyConnectionNumber;
        backendConfiguration.proxy_connection_number_incremental = uiConfiguration.proxyConnectionNumberIncremental;
        backendConfiguration.thread_number = uiConfiguration.agentThreadNumber;
        backendConfiguration.proxy_connection_check_interval_seconds = uiConfiguration.proxyConnectionCheckInterval;
        backendConfiguration.proxy_connection_check_timeout = uiConfiguration.proxyConnectionCheckTimeout

        return from(invoke<any>(SAVE_AGENT_CONFIGURATION_BACKEND_COMMAND, {
            configuration: {
                port: backendConfiguration.port,
                proxy_addresses: backendConfiguration.proxy_addresses,
                user_token: backendConfiguration.user_token,
                compress: backendConfiguration.compress,
                client_buffer_size: backendConfiguration.client_buffer_size,
                message_framed_buffer_size: backendConfiguration.message_framed_buffer_size,
                init_proxy_connection_number: backendConfiguration.init_proxy_connection_number,
                min_proxy_connection_number: backendConfiguration.min_proxy_connection_number,
                proxy_connection_number_incremental: backendConfiguration.proxy_connection_number_incremental,
                thread_number: backendConfiguration.thread_number,
                proxy_connection_check_interval_seconds: backendConfiguration.proxy_connection_check_interval_seconds,
                proxy_connection_check_timeout: backendConfiguration.proxy_connection_check_timeout
            }
        }));
    }

    public startAgentServer(): Observable<any> {
        let thisObject = this;
        return from(invoke<any>(START_AGENT_SERVER_BACKEND_COMMAND)).pipe(map(data => {
            thisObject.agentStarted = true;
            data
        }));
    }

    public stopAgentServer(): Observable<any> {
        return from(invoke<any>(STOP_AGENT_SERVER_BACKEND_COMMAND));
    }

    public loadAgentServerStatus(): Observable<BackendAgentServerStatus> {
        return from(invoke<BackendAgentServerStatus>(RETRIVE_AGENT_SERVER_STATUS_BACKEND_COMMAND));
    }
}
