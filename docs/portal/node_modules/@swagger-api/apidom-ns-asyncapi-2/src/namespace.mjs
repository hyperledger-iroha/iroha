/**
 * AsyncApi >= 2.0.0 <=2.6.0 specification elements.
 */
import AsyncApi2Element from "./elements/AsyncApi2.mjs";
import AsyncApiVersionElement from "./elements/AsyncApiVersion.mjs";
import ChannelBindingsElement from "./elements/ChannelBindings.mjs";
import ChannelItemElement from "./elements/ChannelItem.mjs";
import ChannelsElement from "./elements/Channels.mjs";
import ComponentsElement from "./elements/Components.mjs";
import ContactElement from "./elements/Contact.mjs";
import CorrelationIDElement from "./elements/CorrelationID.mjs";
import DefaultContentTypeElement from "./elements/DefaultContentType.mjs";
import ExternalDocumentationElement from "./elements/ExternalDocumentation.mjs";
import IdentifierElement from "./elements/Identifier.mjs";
import InfoElement from "./elements/Info.mjs";
import LicenseElement from "./elements/License.mjs";
import MessageElement from "./elements/Message.mjs";
import MessageBindingsElement from "./elements/MessageBindings.mjs";
import MessageExampleElement from "./elements/MessageExample.mjs";
import MessageTraitElement from "./elements/MessageTrait.mjs";
import OAuthFlowElement from "./elements/OAuthFlow.mjs";
import OAuthFlowsElement from "./elements/OAuthFlows.mjs";
import OperationElement from "./elements/Operation.mjs";
import OperationBindingsElement from "./elements/OperationBindings.mjs";
import OperationTraitElement from "./elements/OperationTrait.mjs";
import ParameterElement from "./elements/Parameter.mjs";
import ParametersElement from "./elements/Parameters.mjs";
import ReferenceElement from "./elements/Reference.mjs";
import SchemaElement from "./elements/Schema.mjs";
import SecurityRequirementElement from "./elements/SecurityRequirement.mjs";
import SecuritySchemeElement from "./elements/SecurityScheme.mjs";
import ServerElement from "./elements/Server.mjs";
import ServerBindingsElement from "./elements/ServerBindings.mjs";
import ServersElement from "./elements/Servers.mjs";
import ServerVariableElement from "./elements/ServerVariable.mjs";
import TagElement from "./elements/Tag.mjs";
import TagsElement from "./elements/Tags.mjs";
/**
 * Binding elements.
 */
// AMQP 0-9-1
import AmqpChannelBindingElement from "./elements/bindings/amqp/AmqpChannelBinding.mjs";
import AmqpMessageBindingElement from "./elements/bindings/amqp/AmqpMessageBinding.mjs";
import AmqpOperationBindingElement from "./elements/bindings/amqp/AmqpOperationBinding.mjs";
import AmqpServerBindingElement from "./elements/bindings/amqp/AmqpServerBinding.mjs"; // AMQP 1.0
import Amqp1ChannelBindingElement from "./elements/bindings/amqp1/Amqp1ChannelBinding.mjs";
import Amqp1MessageBindingElement from "./elements/bindings/amqp1/Amqp1MessageBinding.mjs";
import Amqp1OperationBindingElement from "./elements/bindings/amqp1/Amqp1OperationBinding.mjs";
import Amqp1ServerBindingElement from "./elements/bindings/amqp1/Amqp1ServerBinding.mjs"; // Anypoint MQ
import AnypointmqChannelBindingElement from "./elements/bindings/anypointmq/AnypointmqChannelBinding.mjs";
import AnypointmqMessageBindingElement from "./elements/bindings/anypointmq/AnypointmqMessageBinding.mjs";
import AnypointmqOperationBindingElement from "./elements/bindings/anypointmq/AnypointmqOperationBinding.mjs";
import AnypointmqServerBindingElement from "./elements/bindings/anypointmq/AnypointmqServerBinding.mjs"; // Google Cloud Pub/Sub
import GooglepubsubChannelBindingElement from "./elements/bindings/googlepubsub/GooglepubsubChannelBinding.mjs";
import GooglepubsubMessageBindingElement from "./elements/bindings/googlepubsub/GooglepubsubMessageBinding.mjs";
import GooglepubsubOperationBindingElement from "./elements/bindings/googlepubsub/GooglepubsubOperationBinding.mjs";
import GooglepubsubServerBindingElement from "./elements/bindings/googlepubsub/GooglepubsubServerBinding.mjs"; // HTTP
import HttpChannelBindingElement from "./elements/bindings/http/HttpChannelBinding.mjs";
import HttpMessageBindingElement from "./elements/bindings/http/HttpMessageBinding.mjs";
import HttpOperationBindingElement from "./elements/bindings/http/HttpOperationBinding.mjs";
import HttpServerBindingElement from "./elements/bindings/http/HttpServerBinding.mjs"; // IBM MQ
import IbmmqChannelBindingElement from "./elements/bindings/ibmmq/IbmmqChannelBinding.mjs";
import IbmmqMessageBindingElement from "./elements/bindings/ibmmq/IbmmqMessageBinding.mjs";
import IbmmqOperationBindingElement from "./elements/bindings/ibmmq/IbmmqOperationBinding.mjs";
import IbmmqServerBindingElement from "./elements/bindings/ibmmq/IbmmqServerBinding.mjs"; // JMS
import JmsChannelBindingElement from "./elements/bindings/jms/JmsChannelBinding.mjs";
import JmsMessageBindingElement from "./elements/bindings/jms/JmsMessageBinding.mjs";
import JmsOperationBindingElement from "./elements/bindings/jms/JmsOperationBinding.mjs";
import JmsServerBindingElement from "./elements/bindings/jms/JmsServerBinding.mjs"; // Kafka
import KafkaChannelBindingElement from "./elements/bindings/kafka/KafkaChannelBinding.mjs";
import KafkaMessageBindingElement from "./elements/bindings/kafka/KafkaMessageBinding.mjs";
import KafkaOperationBindingElement from "./elements/bindings/kafka/KafkaOperationBinding.mjs";
import KafkaServerBindingElement from "./elements/bindings/kafka/KafkaServerBinding.mjs"; // Mercure
import MercureChannelBindingElement from "./elements/bindings/mercure/MercureChannelBinding.mjs";
import MercureMessageBindingElement from "./elements/bindings/mercure/MercureMessageBinding.mjs";
import MercureOperationBindingElement from "./elements/bindings/mercure/MercureOperationBinding.mjs";
import MercureServerBindingElement from "./elements/bindings/mercure/MercureServerBinding.mjs"; // MQTT
import MqttChannelBindingElement from "./elements/bindings/mqtt/MqttChannelBinding.mjs";
import MqttMessageBindingElement from "./elements/bindings/mqtt/MqttMessageBinding.mjs";
import MqttOperationBindingElement from "./elements/bindings/mqtt/MqttOperationBinding.mjs";
import MqttServerBindingElement from "./elements/bindings/mqtt/MqttServerBinding.mjs"; // MQTT 5
import Mqtt5ChannelBindingElement from "./elements/bindings/mqtt5/Mqtt5ChannelBinding.mjs";
import Mqtt5MessageBindingElement from "./elements/bindings/mqtt5/Mqtt5MessageBinding.mjs";
import Mqtt5OperationBindingElement from "./elements/bindings/mqtt5/Mqtt5OperationBinding.mjs";
import Mqtt5ServerBindingElement from "./elements/bindings/mqtt5/Mqtt5ServerBinding.mjs"; // NATS
import NatsChannelBindingElement from "./elements/bindings/nats/NatsChannelBinding.mjs";
import NatsMessageBindingElement from "./elements/bindings/nats/NatsMessageBinding.mjs";
import NatsOperationBindingElement from "./elements/bindings/nats/NatsOperationBinding.mjs";
import NatsServerBindingElement from "./elements/bindings/nats/NatsServerBinding.mjs"; // Pulsar
import PulsarChannelBindingElement from "./elements/bindings/pulsar/PulsarChannelBinding.mjs";
import PulsarMessageBindingElement from "./elements/bindings/pulsar/PulsarMessageBinding.mjs";
import PulsarOperationBindingElement from "./elements/bindings/pulsar/PulsarOperationBinding.mjs";
import PulsarServerBindingElement from "./elements/bindings/pulsar/PulsarServerBinding.mjs"; // Redis
import RedisChannelBindingElement from "./elements/bindings/redis/RedisChannelBinding.mjs";
import RedisMessageBindingElement from "./elements/bindings/redis/RedisMessageBinding.mjs";
import RedisOperationBindingElement from "./elements/bindings/redis/RedisOperationBinding.mjs";
import RedisServerBindingElement from "./elements/bindings/redis/RedisServerBinding.mjs"; // SNS
import SnsChannelBindingElement from "./elements/bindings/sns/SnsChannelBinding.mjs";
import SnsMessageBindingElement from "./elements/bindings/sns/SnsMessageBinding.mjs";
import SnsOperationBindingElement from "./elements/bindings/sns/SnsOperationBinding.mjs";
import SnsServerBindingElement from "./elements/bindings/sns/SnsServerBinding.mjs"; // Solace
import SolaceChannelBindingElement from "./elements/bindings/solace/SolaceChannelBinding.mjs";
import SolaceMessageBindingElement from "./elements/bindings/solace/SolaceMessageBinding.mjs";
import SolaceOperationBindingElement from "./elements/bindings/solace/SolaceOperationBinding.mjs";
import SolaceServerBindingElement from "./elements/bindings/solace/SolaceServerBinding.mjs"; // SQS
import SqsChannelBindingElement from "./elements/bindings/sqs/SqsChannelBinding.mjs";
import SqsMessageBindingElement from "./elements/bindings/sqs/SqsMessageBinding.mjs";
import SqsOperationBindingElement from "./elements/bindings/sqs/SqsOperationBinding.mjs";
import SqsServerBindingElement from "./elements/bindings/sqs/SqsServerBinding.mjs"; // STOMP
import StompChannelBindingElement from "./elements/bindings/stomp/StompChannelBinding.mjs";
import StompMessageBindingElement from "./elements/bindings/stomp/StompMessageBinding.mjs";
import StompOperationBindingElement from "./elements/bindings/stomp/StompOperationBinding.mjs";
import StompServerBindingElement from "./elements/bindings/stomp/StompServerBinding.mjs"; // WebSocket
import WebSocketChannelBindingElement from "./elements/bindings/ws/WebSocketChannelBinding.mjs";
import WebSocketMessageBindingElement from "./elements/bindings/ws/WebSocketMessageBinding.mjs";
import WebSocketOperationBindingElement from "./elements/bindings/ws/WebSocketOperationBinding.mjs";
import WebSocketServerBindingElement from "./elements/bindings/ws/WebSocketServerBinding.mjs";
/**
 * @public
 */
const asyncApi2 = {
  namespace: options => {
    const {
      base
    } = options;

    /**
     * AsyncApi >= 2.0.0 <=2.6.0 specification elements.
     */
    base.register('asyncApi2', AsyncApi2Element);
    base.register('asyncApiVersion', AsyncApiVersionElement);
    base.register('channelBindings', ChannelBindingsElement);
    base.register('channelItem', ChannelItemElement);
    base.register('channels', ChannelsElement);
    base.register('components', ComponentsElement);
    base.register('contact', ContactElement);
    base.register('correlationID', CorrelationIDElement);
    base.register('defaultContentType', DefaultContentTypeElement);
    base.register('externalDocumentation', ExternalDocumentationElement);
    base.register('identifier', IdentifierElement);
    base.register('info', InfoElement);
    base.register('license', LicenseElement);
    base.register('message', MessageElement);
    base.register('messageBindings', MessageBindingsElement);
    base.register('messageExample', MessageExampleElement);
    base.register('messageTrait', MessageTraitElement);
    base.register('oAuthFlow', OAuthFlowElement);
    base.register('oAuthFlows', OAuthFlowsElement);
    base.register('operation', OperationElement);
    base.register('operationBindings', OperationBindingsElement);
    base.register('operationTrait', OperationTraitElement);
    base.register('parameter', ParameterElement);
    base.register('parameters', ParametersElement);
    base.register('reference', ReferenceElement);
    base.register('schema', SchemaElement);
    base.register('securityRequirement', SecurityRequirementElement);
    base.register('securityScheme', SecuritySchemeElement);
    base.register('server', ServerElement);
    base.register('serverBindings', ServerBindingsElement);
    base.register('servers', ServersElement);
    base.register('serverVariable', ServerVariableElement);
    base.register('tag', TagElement);
    base.register('tags', TagsElement);

    /**
     * Binding elements.
     */
    // AMQP 0-9-1
    base.register('amqpChannelBinding', AmqpChannelBindingElement);
    base.register('amqpMessageBinding', AmqpMessageBindingElement);
    base.register('amqpOperationBinding', AmqpOperationBindingElement);
    base.register('amqpServerBinding', AmqpServerBindingElement);
    // AMQP 1.0
    base.register('amqp1ChannelBinding', Amqp1ChannelBindingElement);
    base.register('amqp1MessageBinding', Amqp1MessageBindingElement);
    base.register('amqp1OperationBinding', Amqp1OperationBindingElement);
    base.register('amqp1ServerBinding', Amqp1ServerBindingElement);
    // Anypoint MQ
    base.register('anypointmqChannelBinding', AnypointmqChannelBindingElement);
    base.register('anypointmqMessageBinding', AnypointmqMessageBindingElement);
    base.register('anypointmqOperationBinding', AnypointmqOperationBindingElement);
    base.register('anypointmqServerBinding', AnypointmqServerBindingElement);
    // Google Cloud Pub/Sub
    base.register('googlepubsubChannelBinding', GooglepubsubChannelBindingElement);
    base.register('googlepubsubMessageBinding', GooglepubsubMessageBindingElement);
    base.register('googlepubsubOperationBinding', GooglepubsubOperationBindingElement);
    base.register('googlepubsubServerBinding', GooglepubsubServerBindingElement);
    // HTTP
    base.register('httpChannelBinding', HttpChannelBindingElement);
    base.register('httpMessageBinding', HttpMessageBindingElement);
    base.register('httpOperationBinding', HttpOperationBindingElement);
    base.register('httpServerBinding', HttpServerBindingElement);
    // IBM MQ
    base.register('ibmmqChannelBinding', IbmmqChannelBindingElement);
    base.register('ibmmqMessageBinding', IbmmqMessageBindingElement);
    base.register('ibmmqOperationBinding', IbmmqOperationBindingElement);
    base.register('ibmmqServerBinding', IbmmqServerBindingElement);
    // JMS
    base.register('jmsChannelBinding', JmsChannelBindingElement);
    base.register('jmsMessageBinding', JmsMessageBindingElement);
    base.register('jmsOperationBinding', JmsOperationBindingElement);
    base.register('jmsServerBinding', JmsServerBindingElement);
    // Kafka
    base.register('kafkaChannelBinding', KafkaChannelBindingElement);
    base.register('kafkaMessageBinding', KafkaMessageBindingElement);
    base.register('kafkaOperationBinding', KafkaOperationBindingElement);
    base.register('kafkaServerBinding', KafkaServerBindingElement);
    // Mercure
    base.register('mercureChannelBinding', MercureChannelBindingElement);
    base.register('mercureMessageBinding', MercureMessageBindingElement);
    base.register('mercureOperationBinding', MercureOperationBindingElement);
    base.register('mercureServerBinding', MercureServerBindingElement);
    // MQTT
    base.register('mqttChannelBinding', MqttChannelBindingElement);
    base.register('mqttMessageBinding', MqttMessageBindingElement);
    base.register('mqttOperationBinding', MqttOperationBindingElement);
    base.register('mqttServerBinding', MqttServerBindingElement);
    // MQTT 5
    base.register('mqtt5ChannelBinding', Mqtt5ChannelBindingElement);
    base.register('mqtt5MessageBinding', Mqtt5MessageBindingElement);
    base.register('mqtt5OperationBinding', Mqtt5OperationBindingElement);
    base.register('mqtt5ServerBinding', Mqtt5ServerBindingElement);
    // NATS
    base.register('natsChannelBinding', NatsChannelBindingElement);
    base.register('natsMessageBinding', NatsMessageBindingElement);
    base.register('natsOperationBinding', NatsOperationBindingElement);
    base.register('natsServerBinding', NatsServerBindingElement);
    // Pulsar
    base.register('pulsarChannelBinding', PulsarChannelBindingElement);
    base.register('pulsarMessageBinding', PulsarMessageBindingElement);
    base.register('pulsarOperationBinding', PulsarOperationBindingElement);
    base.register('pulsarServerBinding', PulsarServerBindingElement);
    // Redis
    base.register('redisChannelBinding', RedisChannelBindingElement);
    base.register('redisMessageBinding', RedisMessageBindingElement);
    base.register('redisOperationBinding', RedisOperationBindingElement);
    base.register('redisServerBinding', RedisServerBindingElement);
    // SNS
    base.register('snsChannelBinding', SnsChannelBindingElement);
    base.register('snsMessageBinding', SnsMessageBindingElement);
    base.register('snsOperationBinding', SnsOperationBindingElement);
    base.register('snsServerBinding', SnsServerBindingElement);
    // Solace
    base.register('solaceChannelBinding', SolaceChannelBindingElement);
    base.register('solaceMessageBinding', SolaceMessageBindingElement);
    base.register('solaceOperationBinding', SolaceOperationBindingElement);
    base.register('solaceServerBinding', SolaceServerBindingElement);
    // SQS
    base.register('sqsChannelBinding', SqsChannelBindingElement);
    base.register('sqsMessageBinding', SqsMessageBindingElement);
    base.register('sqsOperationBinding', SqsOperationBindingElement);
    base.register('sqsServerBinding', SqsServerBindingElement);
    // STOMP
    base.register('stompChannelBinding', StompChannelBindingElement);
    base.register('stompMessageBinding', StompMessageBindingElement);
    base.register('stompOperationBinding', StompOperationBindingElement);
    base.register('stompServerBinding', StompServerBindingElement);
    // WebSocket
    base.register('webSocketChannelBinding', WebSocketChannelBindingElement);
    base.register('webSocketMessageBinding', WebSocketMessageBindingElement);
    base.register('webSocketOperationBinding', WebSocketOperationBindingElement);
    base.register('webSocketServerBinding', WebSocketServerBindingElement);
    return base;
  }
};
export default asyncApi2;