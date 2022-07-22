import { ChangeDetectorRef, Component, OnInit } from '@angular/core';
import { BackendService } from 'src/app/service/backend.service';
import UiAgentConfiguration from 'src/app/dto/UiAgentConfiguration';
import { MessageService } from 'primeng/api';

@Component({
    templateUrl: './settings.component.html',
    styleUrls: ['./settings.component.css']
})
export class SettingsComponent implements OnInit {
    userToken: string | undefined;
    proxyAddressesAsString: string | undefined;
    listingingPort: string | undefined;
    clientBufferSize = 0;
    messageFramedBufferSize = 0;
    initProxyConnectionNumber = 0;
    minProxyConnectionNumber = 0;
    proxyConnectionNumberIncremental = 0;
    agentThreadNumber = 0;
    enableCompressing: boolean = false;
    agentStarted: boolean = false;


    constructor(private changeRef: ChangeDetectorRef, private messageService: MessageService, private backendService: BackendService,) { }

    ngOnInit() {
        let thisObject = this;
        this.backendService.loadAgentServerStatus().subscribe({
            next(value) {
                if (value.status == "STARTED") {
                    thisObject.agentStarted = true;
                    thisObject.messageService.add(
                        {
                            severity: 'success',
                            summary: 'Agent information',
                            detail: 'Agent server started already'
                        })
                    return;
                }
                if (value.status == "STOPPED") {
                    thisObject.agentStarted = false;
                    thisObject.messageService.add(
                        {
                            severity: 'warn',
                            summary: 'Agent information',
                            detail: 'Agent server stopped already'
                        })
                }
            },
            error(e) {
                thisObject.messageService.add(
                    {
                        severity: 'error',
                        summary: 'Agent error',
                        detail: e
                    })
            }
        })
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
                thisObject.messageService.add(
                    {
                        severity: 'error',
                        summary: 'Agent error',
                        detail: e
                    })
            }
        });
        this.backendService.listenToAgentServerStart(event => {
            this.agentStarted = true;
            thisObject.messageService.add(
                {
                    severity: 'success',
                    summary: 'Agent information',
                    detail: 'Agent server started already'
                })
            this.changeRef.detectChanges();
        });
        this.backendService.listenToAgentServerStop(event => {
            this.agentStarted = false;
            thisObject.messageService.add(
                {
                    severity: 'warn',
                    summary: 'Agent information',
                    detail: 'Agent server stopped already'
                })
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
            error(e) {
                this.messageService.add(
                    {
                        severity: 'error',
                        summary: 'Agent error',
                        detail: e
                    })
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
                    error(e) {
                        thisObject.messageService.add(
                            {
                                severity: 'error',
                                summary: 'Agent error',
                                detail: e
                            })
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
            error(e) {
                thisObject.messageService.add(
                    {
                        severity: 'error',
                        summary: 'Agent error',
                        detail: e
                    })
            },
        })
    }

}
