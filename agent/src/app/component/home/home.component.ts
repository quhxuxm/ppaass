import { BackendService } from './../../service/backend.service';
import { ChangeDetectorRef, Component, OnInit } from '@angular/core';
import UiAgentConfiguration from 'src/app/dto/UiAgentConfiguration';
import { faCirclePause, faCirclePlay, faNetworkWired, faServer, faUser } from '@fortawesome/free-solid-svg-icons';

@Component({
    selector: 'app-home',
    templateUrl: './home.component.html',
    styleUrls: ['./home.component.scss']
})
export class HomeComponent implements OnInit {
    userToken: string | undefined;
    proxyAddresses: string[] | undefined;
    listingingPort: string | undefined;
    agentServerStarted: boolean;
    userInfoPannelOpenState: boolean;
    proxyInfoPannelOpenState: boolean;
    agentInfoPannelOpenState: boolean;
    iconUser = faUser;
    iconProxy = faNetworkWired;
    iconAgent = faServer;
    iconStart = faCirclePlay;
    iconStop = faCirclePause;

    constructor(private changeRef: ChangeDetectorRef, private backendService: BackendService) {
        this.agentServerStarted = false;
        this.userInfoPannelOpenState = false;
        this.proxyInfoPannelOpenState = false;
        this.agentInfoPannelOpenState = false;
    }

    ngOnInit(): void {
        let thisObject = this;
        this.backendService.loadAgentConfiguration().subscribe({
            next(uiConfiguration) {
                thisObject.userToken = uiConfiguration.userToken;
                thisObject.proxyAddresses = uiConfiguration.proxyAddresses;
                thisObject.listingingPort = uiConfiguration.listeningPort;
            },
            error(e) {
            }
        });
        this.backendService.listenToAgentServerStart(event => {
            this.agentServerStarted = true;
            this.changeRef.detectChanges();
        });
        this.backendService.listenToAgentServerStop(event => {
            this.agentServerStarted = false;
            this.changeRef.detectChanges();
        });
    }

    private saveAgentServerConfiguration(successCallback: () => void) {
        let configuration = new UiAgentConfiguration();
        configuration.listeningPort = this.listingingPort;
        configuration.proxyAddresses = this.proxyAddresses;
        configuration.userToken = this.userToken;
        this.backendService.saveAgentConfiguration(configuration).subscribe({
            next(saveResult) {
                successCallback();
            },
            error(err) {
                alert(`Fail to save agent configuration because of error: ${err}`);
            },
        });
    }

    public operateAgentServer() {
        let thisObject = this;
        if (!this.agentServerStarted) {
            this.saveAgentServerConfiguration(() => {
                this.backendService.startAgentServer().subscribe({
                    next(value) {
                        thisObject.agentServerStarted = true;
                    },
                    error(err) {
                        alert(`Fail to start agent server because of error: ${err}`);
                    },
                })
            })
        } else {
            this.stopAgentServer();
        }
    }

    private stopAgentServer() {
        let thisObject = this;
        this.backendService.stopAgentServer().subscribe({
            next(value) {
                thisObject.agentServerStarted = false;
            },
            error(err) {
                alert(`Fail to start agent server because of error: ${err}`);
            },
        })
    }

}
