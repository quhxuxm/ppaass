import { BackendService } from './../../service/backend.service';
import { ChangeDetectorRef, Component, OnInit } from '@angular/core';
import UiAgentConfiguration from 'src/app/dto/UiAgentConfiguration';

@Component({
    selector: 'app-home',
    templateUrl: './home.component.html',
    styleUrls: ['./home.component.scss']
})
export class HomeComponent implements OnInit {
    public userToken: string | undefined;
    public proxyAddresses: string[] | undefined;
    public listingingPort: number | undefined;
    public agentServerStarted: boolean;

    constructor(private changeRef: ChangeDetectorRef, private backendService: BackendService) {
        this.agentServerStarted = false;
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

    public saveAgentServerConfiguration() {
        let configuration = new UiAgentConfiguration();
        configuration.listeningPort = this.listingingPort;
        configuration.proxyAddresses = this.proxyAddresses;
        configuration.userToken = this.userToken;
        this.backendService.saveAgentConfiguration(configuration).subscribe({
            next(saveResult) {

            },
            error(err) {
                alert(`Fail to save agent configuration because of error: ${err}`);
            },
        });
    }

    public startAgentServer() {
        let thisObject = this;
        this.backendService.startAgentServer().subscribe({
            next(value) {
                thisObject.agentServerStarted = true;
            },
            error(err) {
                alert(`Fail to start agent server because of error: ${err}`);
            },
        })
    }

    public stopAgentServer() {
        let thisObject = this;
        this.backendService.startAgentServer().subscribe({
            next(value) {
                thisObject.agentServerStarted = false;
            },
            error(err) {
                alert(`Fail to start agent server because of error: ${err}`);
            },
        })
    }

}
