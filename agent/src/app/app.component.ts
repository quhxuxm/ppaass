import { ChangeDetectionStrategy, ChangeDetectorRef, Component, Input, OnInit, Output } from '@angular/core';
import { invoke } from '@tauri-apps/api/tauri'
import { listen, UnlistenFn } from '@tauri-apps/api/event';

@Component({
    selector: 'app-root',
    templateUrl: './app.component.html',
    styleUrls: ['./app.component.scss'],
    changeDetection: ChangeDetectionStrategy.Default
})
export class AppComponent implements OnInit {
    public userToken: string;
    public proxyServerAddresses: string;
    public enableCompressing: boolean;
    public disableStartButton: boolean;
    public disableStopButton: boolean;
    private agentServerStartEventListener: Promise<UnlistenFn>;
    private agentServerStopEventListener: Promise<UnlistenFn>;


    constructor(private changeRef: ChangeDetectorRef) {
        this.userToken = "";
        this.proxyServerAddresses = "";
        this.enableCompressing = false;
        this.disableStartButton = false;
        this.disableStopButton = true;
        this.agentServerStartEventListener = listen<boolean>('agent-server-start', (event) => {
            this.disableStartButton = true;
            this.disableStopButton = false;
            this.changeRef.detectChanges();
        });
        this.agentServerStopEventListener = listen<boolean>('agent-server-stop', (event) => {
            this.disableStartButton = false;
            this.disableStopButton = true;
            this.changeRef.detectChanges();
        });
    }

    ngOnInit(): void {

    }

    saveConfiguration(): void {
        let commandPayload = {
            uiConfiguration: {
                user_token: this.userToken,
                proxy_addresses: this.proxyServerAddresses.split(";")
            }
        };
        invoke("save_agent_server_config", commandPayload).then((response) => {
            console.log(`${response}`);
        }).catch((exception) => {
            console.log(`Error: ${exception}`);
        });
    }

    startAgentServer(): void {
        invoke("start_agent_server").then((response) => {
            console.log(`${response}`);
            this.disableStartButton = true;
            this.disableStopButton = false;
        }).catch((exception) => {
            console.log(`Error: ${exception}`);
        });

    }

    stopAgentStop(): void {
        invoke("stop_agent_server").then((response) => {
            console.log(`${response}`);
            this.disableStartButton = false;
            this.disableStopButton = true;
        }).catch((exception) => {
            console.log(`Error: ${exception}`);
        });

    }
}
