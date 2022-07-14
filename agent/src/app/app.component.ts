import { ChangeDetectionStrategy, ChangeDetectorRef, Component, Input, OnInit, Output } from '@angular/core';
import { invoke } from '@tauri-apps/api/tauri'
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { appWindow } from '@tauri-apps/api/window';

@Component({
    selector: 'app-root',
    templateUrl: './app.component.html',
    styleUrls: ['./app.component.scss'],
    changeDetection: ChangeDetectionStrategy.Default
})
export class AppComponent implements OnInit {
    public userToken: string;
    public proxyServerAddresses: string;
    public listeningPort: string;

    public enableCompressing: boolean;
    public disableStartButton: boolean;
    public disableStopButton: boolean;

    constructor(private changeRef: ChangeDetectorRef) {

        this.listeningPort = "";
        this.userToken = "";
        this.proxyServerAddresses = "";
        this.enableCompressing = false;
        this.disableStartButton = false;
        this.disableStopButton = true;
    }

    ngOnInit(): void {
        invoke("retrive_agent_configuration").then((response: any) => {
            console.log(`:retrive_agent_configuration: ${response}`);
            this.userToken = response.user_token;
            this.proxyServerAddresses = response.proxy_addresses.toString();
            this.enableCompressing = response.compress;
            this.listeningPort = response.port.toString();
        }).catch((exception) => {
            console.log(`retrive_agent_configuration error: ${exception}`);
        });
        appWindow.listen<boolean>('agent-server-stop', (event) => {
            this.disableStartButton = false;
            this.disableStopButton = true;
            this.changeRef.detectChanges();
        });
        appWindow.listen<boolean>('agent-server-start', (event) => {
            this.disableStartButton = true;
            this.disableStopButton = false;
            this.changeRef.detectChanges();
        });
    }

    saveConfiguration(): void {
        let commandPayload = {
            uiConfiguration: {
                user_token: this.userToken,
                proxy_addresses: this.proxyServerAddresses.split(","),
                compress: this.enableCompressing,
                port: this.listeningPort
            }
        };
        invoke("save_agent_server_config", commandPayload).then((response) => {
            console.log(`save_agent_server_config: ${response}`);
        }).catch((exception) => {
            console.log(`save_agent_server_config error: ${exception}`);
        });
    }

    startAgentServer(): void {
        invoke("start_agent_server").then((response) => {
            console.log(`start_agent_server: ${response}`);
            this.disableStartButton = true;
            this.disableStopButton = false;
        }).catch((exception) => {
            console.log(`start_agent_server error: ${exception}`);
        });

    }

    stopAgentStop(): void {
        invoke("stop_agent_server").then((response) => {
            console.log(`stop_agent_server: ${response}`);
            this.disableStartButton = false;
            this.disableStopButton = true;
        }).catch((exception) => {
            console.log(`stop_agent_server error: ${exception}`);
        });

    }
}
