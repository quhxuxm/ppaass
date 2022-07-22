import { ChangeDetectorRef, Component, OnInit } from '@angular/core';
import { BackendService } from 'src/app/service/backend.service';
import UiAgentConfiguration from 'src/app/dto/UiAgentConfiguration';

@Component({
    templateUrl: './settings.component.html',
    styleUrls: ['./settings.component.css']
})
export class SettingsComponent implements OnInit {
    userToken: string | undefined;
    proxyAddressesAsString: string | undefined;
    listingingPort: string | undefined;
    clientBufferSize = 50;
    messageFramedBufferSize = 50;
    initProxyConnectionNumber = 64;
    minProxyConnectionNumber = 64;
    proxyConnectionNumberIncremental = 64;
    agentThreadNumber = 1024;
    enableCompressing: boolean = true;
    agentStarted: boolean = false;


    constructor(private changeRef: ChangeDetectorRef, private backendService: BackendService) { }

    ngOnInit() {
        let thisObject = this;
        this.backendService.loadAgentConfiguration().subscribe({
            next(uiConfiguration) {
                thisObject.userToken = uiConfiguration.userToken;
                thisObject.proxyAddressesAsString = uiConfiguration.proxyAddresses.toString();
                thisObject.listingingPort = uiConfiguration.listeningPort;
                thisObject.clientBufferSize = uiConfiguration.clientBufferSize;
                thisObject.messageFramedBufferSize = uiConfiguration.messageFramedBufferSize;
                thisObject.initProxyConnectionNumber = uiConfiguration.initProxyConnectionNumber;
                thisObject.minProxyConnectionNumber = uiConfiguration.minProxyConnectionNumber;
                thisObject.proxyConnectionNumberIncremental = uiConfiguration.proxyConnectionNumberIncremental;
                thisObject.enableCompressing = uiConfiguration.enableCompressing;
                thisObject.agentThreadNumber = uiConfiguration.agentThreadNumber;
            },
            error(e) {
            }
        });
        this.backendService.listenToAgentServerStart(event => {
            this.agentStarted = true;
            this.changeRef.detectChanges();
        });
        this.backendService.listenToAgentServerStop(event => {
            this.agentStarted = false;
            this.changeRef.detectChanges();
        });
    }

    private saveAgentServerConfiguration(successCallback: () => void) {
        let configuration = new UiAgentConfiguration();
        configuration.listeningPort = this.listingingPort;
        configuration.proxyAddresses = this.proxyAddressesAsString.split(",");
        configuration.userToken = this.userToken;
        configuration.agentThreadNumber = this.agentThreadNumber;
        configuration.clientBufferSize = this.clientBufferSize;
        configuration.messageFramedBufferSize = this.messageFramedBufferSize;
        configuration.enableCompressing = this.enableCompressing;
        configuration.initProxyConnectionNumber = this.initProxyConnectionNumber;
        configuration.minProxyConnectionNumber = this.minProxyConnectionNumber;
        configuration.proxyConnectionNumberIncremental = this.proxyConnectionNumberIncremental;

        this.backendService.saveAgentConfiguration(configuration).subscribe({
            next(saveResult) {
                successCallback();
            },
            error(err) {
                alert(`Fail to save agent configuration because of error: ${err}`);
            },
        });
    }

    public startAgentServer() {
        let thisObject = this;
        if (!this.agentStarted) {
            this.saveAgentServerConfiguration(() => {
                this.backendService.startAgentServer().subscribe({
                    next(value) {
                        thisObject.agentStarted = true;
                    },
                    error(err) {
                        alert(`Fail to start agent server because of error: ${err}`);
                    },
                })
            })
        }
    }

    public stopAgentServer() {
        let thisObject = this;
        this.backendService.stopAgentServer().subscribe({
            next(value) {
                thisObject.agentStarted = false;
            },
            error(err) {
                alert(`Fail to start agent server because of error: ${err}`);
            },
        })
    }

}
