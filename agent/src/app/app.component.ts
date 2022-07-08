import { Component, OnInit } from '@angular/core';
import { invoke } from '@tauri-apps/api/tauri'

@Component({
    selector: 'app-root',
    templateUrl: './app.component.html',
    styleUrls: ['./app.component.scss']
})
export class AppComponent implements OnInit {
    public userToken: string;
    public proxyServerAddresses: string;
    public enableCompressing: boolean;
    public disableStartButton: boolean;
    constructor() {
        this.userToken = "";
        this.proxyServerAddresses = "";
        this.enableCompressing = false;
        this.disableStartButton = false;
    }
    ngOnInit(): void {
    }

    startAgentServer(): void {
        this.disableStartButton = true;
        invoke("start_agent_server");
    }

    stopAgentStop(): void {
        invoke("stop_agent_server");
        this.disableStartButton = false;
    }
}
