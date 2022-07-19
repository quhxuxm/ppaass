import { BackendAgentConfiguration } from '../dto/BackendAgentConfiguration';
import { Injectable } from '@angular/core';
import { invoke } from '@tauri-apps/api';
import { Event } from '@tauri-apps/api/event';
import { appWindow } from '@tauri-apps/api/window';
import { from, map, Observable } from 'rxjs';
import UiAgentConfiguration from '../dto/UiAgentConfiguration';

const RETRIVE_AGENT_CONFIGURATION_BACKEND_COMMAND = "retrive_agent_configuration";
const SAVE_AGENT_CONFIGURATION_BACKEND_COMMAND = "save_agent_server_config";
const START_AGENT_SERVER_BACKEND_COMMAND = "start_agent_server";
const STOP_AGENT_SERVER_BACKEND_COMMAND = "stop_agent_server";
const AGENT_SERVER_STOP_BACKEND_EVENT = "agent-server-stop-backend-event";
const AGENT_SERVER_START_BACKEND_EVENT = "agent-server-start-backend-event";

@Injectable({
    providedIn: 'root'
})
export class BackendService {

    constructor() { }

    public loadAgentConfiguration(): Observable<UiAgentConfiguration> {
        return from(invoke<BackendAgentConfiguration>(RETRIVE_AGENT_CONFIGURATION_BACKEND_COMMAND)).pipe(map(item => {
            let uiConfiguration = new UiAgentConfiguration();
            uiConfiguration.listeningPort = item.port;
            uiConfiguration.proxyAddresses = item.proxy_addresses;
            uiConfiguration.userToken = item.user_token;
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

        return from(invoke<any>(SAVE_AGENT_CONFIGURATION_BACKEND_COMMAND, {
            configuration: {
                port: backendConfiguration.port,
                proxy_addresses: backendConfiguration.proxy_addresses,
                user_token: backendConfiguration.user_token
            }
        }));
    }

    public startAgentServer(): Observable<any> {
        return from(invoke<any>(START_AGENT_SERVER_BACKEND_COMMAND));
    }

    public stopAgentServer(): Observable<any> {
        return from(invoke<any>(STOP_AGENT_SERVER_BACKEND_COMMAND));
    }
}
